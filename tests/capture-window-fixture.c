#define _POSIX_C_SOURCE 200809L
#include <X11/Xlib.h>
#include <X11/Xatom.h>
#include <X11/extensions/Xcomposite.h>
#include <stdio.h>
#include <string.h>
#include <unistd.h>

int main(int argc, char **argv) {
    if (argc != 2) return 2;
    Display *display = XOpenDisplay(NULL);
    if (!display) return 2;
    int screen = DefaultScreen(display);
    /* Colored WM frame must not contaminate the black client's verdict. */
    Window frame = XCreateSimpleWindow(display, RootWindow(display, screen), 10, 10,
                                       280, 220, 0, 0, 0xff00ff);
    Window window = XCreateSimpleWindow(display, frame, 10, 10, 256, 192, 0, 0, 0);
    Atom property = XInternAtom(display, "_NET_WM_PID", False);
    unsigned long pid = (unsigned long)getpid();
    XChangeProperty(display, window, property, XA_CARDINAL, 32, PropModeReplace,
                    (unsigned char *)&pid, 1);
    if (!strcmp(argv[1], "composite")) XCompositeRedirectWindow(display, window, CompositeRedirectAutomatic);
    XMapWindow(display, frame);
    if (strcmp(argv[1], "unmapped")) XMapWindow(display, window);
    XSync(display, False);
    GC gc = XCreateGC(display, window, 0, NULL);
    if (!strcmp(argv[1], "uniform")) {
        XSetForeground(display, gc, 0xffffff); XFillRectangle(display, window, gc, 0, 0, 256, 192);
    } else if (!strcmp(argv[1], "color") || !strcmp(argv[1], "composite")) {
        XSetForeground(display, gc, 0xffffff); XFillRectangle(display, window, gc, 0, 0, 128, 192);
        XSetForeground(display, gc, 0xff0000); XFillRectangle(display, window, gc, 128, 0, 128, 192);
    }
    XSync(display, False);
    puts("ready"); fflush(stdout);
    sleep(30); XCloseDisplay(display); return 0;
}
