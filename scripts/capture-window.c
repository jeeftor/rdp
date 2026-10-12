#define _POSIX_C_SOURCE 200809L
#include <X11/Xlib.h>
#include <X11/Xatom.h>
#include <X11/Xutil.h>
#ifdef HAVE_XCOMPOSITE
#include <X11/extensions/Xcomposite.h>
#endif
#include <png.h>
#include <errno.h>
#include <fcntl.h>
#include <signal.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/stat.h>
#include <unistd.h>

static int xerror;
static int handle_xerror(Display *display, XErrorEvent *event) {
    (void)display; (void)event; xerror = 1; return 0;
}
static int handle_ioerror(Display *display) {
    (void)display;
    static const char result[] = "{\"status\":\"unavailable\",\"reason\":\"X11 display disconnected during capture\"}\n";
    (void)write(STDOUT_FILENO, result, sizeof(result) - 1); _exit(2);
}
static void timed_out(int signal_number) {
    (void)signal_number;
    static const char result[] = "{\"status\":\"unavailable\",\"reason\":\"X11 capture timed out\"}\n";
    (void)write(STDOUT_FILENO, result, sizeof(result) - 1); _exit(2);
}
static int unavailable(const char *reason) {
    printf("{\"status\":\"unavailable\",\"reason\":\"%s\"}\n", reason); return 2;
}
static void json_string(const char *value) {
    putchar('"');
    for (const unsigned char *p = (const unsigned char *)value; *p; p++) {
        if (*p == '"' || *p == '\\') printf("\\%c", *p);
        else if (*p < 32) printf("\\u%04x", *p);
        else putchar(*p);
    }
    putchar('"');
}
/* Traverse WM frames too, but select only client windows declaring this PID. */
static void find_window(Display *display, Window window, Atom property, unsigned long pid,
                        Window *best, unsigned long *best_area, unsigned int depth, unsigned int *visited) {
    if (depth > 32 || ++*visited > 10000) return;
    Atom type; int format; unsigned long count, remaining; unsigned char *data = NULL;
    if (XGetWindowProperty(display, window, property, 0, 1, False, XA_CARDINAL,
                           &type, &format, &count, &remaining, &data) == Success &&
        type == XA_CARDINAL && format == 32 && count == 1 && data &&
        *(unsigned long *)data == pid) {
        XWindowAttributes attrs;
        if (XGetWindowAttributes(display, window, &attrs) && attrs.map_state == IsViewable &&
            attrs.class == InputOutput && attrs.width > 16 && attrs.height > 16) {
            unsigned long area = (unsigned long)attrs.width * (unsigned long)attrs.height;
            if (area > *best_area) { *best = window; *best_area = area; }
        }
    }
    if (data) XFree(data);
    Window root, parent, *children = NULL; unsigned int length = 0;
    if (XQueryTree(display, window, &root, &parent, &children, &length)) {
        for (unsigned int i = 0; i < length; i++)
            find_window(display, children[i], property, pid, best, best_area, depth + 1, visited);
    }
    if (children) XFree(children);
}
static unsigned char component(unsigned long pixel, unsigned long mask) {
    if (!mask) return 0;
    unsigned int shift = 0;
    while (!(mask & 1)) { mask >>= 1; shift++; }
    return (unsigned char)(((pixel >> shift) & mask) * 255 / mask);
}
static int save_png(const char *path, unsigned char *pixels, unsigned int width, unsigned int height) {
    int descriptor = open(path, O_WRONLY | O_CREAT | O_EXCL | O_NOFOLLOW, 0600);
    if (descriptor < 0) return 0;
    if (fchmod(descriptor, 0600)) { close(descriptor); unlink(path); return 0; }
    FILE *file = fdopen(descriptor, "wb");
    if (!file) { close(descriptor); unlink(path); return 0; }
    png_structp png = png_create_write_struct(PNG_LIBPNG_VER_STRING, NULL, NULL, NULL);
    png_infop info = png ? png_create_info_struct(png) : NULL;
    if (!png || !info) {
        if (png) png_destroy_write_struct(&png, NULL);
        fclose(file); unlink(path); return 0;
    }
    if (setjmp(png_jmpbuf(png))) {
        png_destroy_write_struct(&png, &info); fclose(file); unlink(path); return 0;
    }
    png_init_io(png, file);
    png_set_IHDR(png, info, width, height, 8, PNG_COLOR_TYPE_RGB, PNG_INTERLACE_NONE,
                 PNG_COMPRESSION_TYPE_DEFAULT, PNG_FILTER_TYPE_DEFAULT);
    png_write_info(png, info);
    for (unsigned int y = 0; y < height; y++) png_write_row(png, pixels + y * width * 3);
    png_write_end(png, info); png_destroy_write_struct(&png, &info);
    if (fclose(file)) { unlink(path); return 0; }
    return 1;
}
int main(int argc, char **argv) {
    if (argc != 5 || strcmp(argv[1], "--pid") || strcmp(argv[3], "--output"))
        return unavailable("Expected --pid PROCESS_ID --output NEW_PNG_PATH");
    char *end; errno = 0; unsigned long pid = strtoul(argv[2], &end, 10);
    if (errno || !pid || *end || argv[2][0] == '-' || pid > 2147483647UL)
        return unavailable("Invalid process ID");
    signal(SIGALRM, timed_out); alarm(3);
    Display *display = XOpenDisplay(NULL);
    if (!display) return unavailable("No accessible X11 display; native Wayland capture needs compositor permission");
    XSetErrorHandler(handle_xerror); XSetIOErrorHandler(handle_ioerror);
    Window window = None; unsigned long area = 0; unsigned int visited = 0;
    Atom property = XInternAtom(display, "_NET_WM_PID", True);
    if (property != None) {
        for (int screen = 0; screen < ScreenCount(display); screen++)
            find_window(display, RootWindow(display, screen), property, pid, &window, &area, 0, &visited);
    }
    XSync(display, False); xerror = 0;
    if (window == None) { XCloseDisplay(display); return unavailable("No mapped X11 client window matches the process ID"); }
    XWindowAttributes attrs;
    if (!XGetWindowAttributes(display, window, &attrs) || attrs.map_state != IsViewable ||
        attrs.width > 8192 || attrs.height > 8192 || area > 32000000UL) {
        XCloseDisplay(display); return unavailable("Matching window closed, unmapped or too large");
    }
    XImage *image = NULL; const char *method = "visible";
#ifdef HAVE_XCOMPOSITE
    int event_base, error_base;
    if (XCompositeQueryExtension(display, &event_base, &error_base)) {
        Pixmap pixmap = XCompositeNameWindowPixmap(display, window);
        XSync(display, False);
        if (!xerror && pixmap != None) {
            image = XGetImage(display, pixmap, 0, 0, (unsigned int)attrs.width,
                              (unsigned int)attrs.height, AllPlanes, ZPixmap);
            XSync(display, False);
            XFreePixmap(display, pixmap);
            if (xerror && image) { XDestroyImage(image); image = NULL; }
            if (image) method = "composite";
        }
        xerror = 0;
    }
#endif
    if (!image) {
        image = XGetImage(display, window, 0, 0, (unsigned int)attrs.width,
                          (unsigned int)attrs.height, AllPlanes, ZPixmap);
        XSync(display, False);
    }
    if (!image || xerror) {
        if (image) XDestroyImage(image);
        XCloseDisplay(display); return unavailable("Matching window cannot be captured");
    }
    if (attrs.visual->class == TrueColor) {
        image->red_mask = attrs.visual->red_mask; image->green_mask = attrs.visual->green_mask; image->blue_mask = attrs.visual->blue_mask;
    }
    if (!image->red_mask || !image->green_mask || !image->blue_mask) {
        XDestroyImage(image); XCloseDisplay(display); return unavailable("Unsupported indexed window pixel format");
    }
    unsigned int width = (unsigned int)attrs.width, height = (unsigned int)attrs.height;
    unsigned char *pixels = malloc((size_t)width * height * 3);
    if (!pixels) { XDestroyImage(image); XCloseDisplay(display); return unavailable("Capture memory allocation failed"); }
    double sum = 0, squares = 0; unsigned long samples = 0, nonblack = 0;
    for (unsigned int y = 0; y < height; y++) {
        for (unsigned int x = 0; x < width; x++) {
            unsigned long pixel = XGetPixel(image, (int)x, (int)y);
            unsigned char r = component(pixel, image->red_mask), g = component(pixel, image->green_mask), b = component(pixel, image->blue_mask);
            size_t offset = ((size_t)y * width + x) * 3;
            pixels[offset] = r; pixels[offset + 1] = g; pixels[offset + 2] = b;
            /* Exclude four client-edge pixels so borders cannot make black appear valid. */
            if (x >= 4 && y >= 4 && x + 4 < width && y + 4 < height) {
                double brightness = (r + g + b) / 3.0;
                samples++; nonblack += r > 16 || g > 16 || b > 16;
                sum += brightness; squares += brightness * brightness;
            }
        }
    }
    XDestroyImage(image); XCloseDisplay(display);
    double ratio = (double)nonblack / samples;
    double mean = sum / samples, variance = squares / samples - mean * mean;
    if (variance < 0) variance = 0;
    int blank = ratio < 0.01 || variance < 4.0;
    if (!save_png(argv[4], pixels, width, height)) { free(pixels); return unavailable("Cannot create private screenshot at the new output path"); }
    free(pixels); alarm(0);
    printf("{\"status\":\"%s\",\"reason\":\"%s\",\"width\":%u,\"height\":%u,\"nonblack_ratio\":%.6f,\"variance\":%.6f,\"capture_method\":\"%s\",\"file\":",
           blank ? "blank" : "content", blank ? "Client pixels are near-black or near-uniform; usability needs visual confirmation" : "Client pixels contain variation; usability needs visual confirmation",
           width, height, ratio, variance, method);
    json_string(argv[4]); puts("}"); return 0;
}
