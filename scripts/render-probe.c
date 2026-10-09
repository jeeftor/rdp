#include <SDL3/SDL.h>
#include <signal.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/wait.h>
#include <unistd.h>

/* Probe in isolated children: a stalled driver must not prevent another backend. */
static int probe(const char *backend, const char *renderer)
{
    setenv("SDL_VIDEO_DRIVER", backend, 1);
    setenv("SDL_VIDEODRIVER", backend, 1);
    setenv("SDL_RENDER_DRIVER", renderer, 1);
    if (!SDL_Init(SDL_INIT_VIDEO)) goto failed;
    SDL_Window *window = SDL_CreateWindow("FreeRDP graphics probe", 320, 240, SDL_WINDOW_HIDDEN);
    if (!window) goto failed;
    SDL_Renderer *render = SDL_CreateRenderer(window, renderer);
    if (!render) goto failed;
    SDL_Texture *target = SDL_CreateTexture(render, SDL_PIXELFORMAT_BGRA32, SDL_TEXTUREACCESS_TARGET, 320, 240);
    if (!target || !SDL_SetRenderTarget(render, target) || !SDL_RenderClear(render)
        || !SDL_SetRenderTarget(render, NULL) || !SDL_RenderTexture(render, target, NULL, NULL)
        || !SDL_RenderPresent(render)) goto failed;
    fprintf(stderr, "FreeRDP graphics: %s/%s passed frame presentation", backend, renderer);
    if (!strcmp(renderer, "opengl") || !strcmp(renderer, "opengles2")) {
        typedef const unsigned char *(*get_string)(unsigned int);
        get_string gl_string = (get_string)SDL_GL_GetProcAddress("glGetString");
        const unsigned char *device = gl_string ? gl_string(0x1F01) : NULL;
        if (device) fprintf(stderr, " (%s)", device);
    }
    fprintf(stderr, "\n");
    SDL_Quit();
    return 0;
failed:
    fprintf(stderr, "FreeRDP graphics: %s/%s failed: %s\n", backend, renderer, SDL_GetError());
    SDL_Quit();
    return 1;
}

static int bounded_probe(const char *backend, const char *renderer)
{
    pid_t child = fork();
    if (child < 0) { perror("FreeRDP graphics fork"); return 1; }
    if (!child) _exit(probe(backend, renderer));
    for (int tick = 0; tick < 60; tick++) {
        int status = 0;
        pid_t result = waitpid(child, &status, WNOHANG);
        if (result == child) return WIFEXITED(status) && WEXITSTATUS(status) == 0 ? 0 : 1;
        if (result < 0) break;
        usleep(50000);
    }
    kill(child, SIGKILL);
    waitpid(child, NULL, 0);
    fprintf(stderr, "FreeRDP graphics: %s/%s timed out after 3 seconds\n", backend, renderer);
    return 1;
}

static const char *value(const char *name)
{
    const char *text = getenv(name);
    return text && *text ? text : NULL;
}

int main(void)
{
    const char *explicit_backend = value("SDL_VIDEO_DRIVER");
    if (!explicit_backend) explicit_backend = value("SDL_VIDEODRIVER");
    const char *explicit_renderer = value("SDL_RENDER_DRIVER");
    const char *backends[] = {"wayland", "x11"};
    const char *renderers[] = {"opengl", "opengles2"};
    for (int b = 0; b < 2; b++) {
        const char *backend = explicit_backend ? explicit_backend : backends[b];
        if (!explicit_backend && !value(b == 0 ? "WAYLAND_DISPLAY" : "DISPLAY")) continue;
        for (int r = 0; r < 2; r++) {
            const char *renderer = explicit_renderer ? explicit_renderer : renderers[r];
            if (bounded_probe(backend, renderer) == 0) {
                printf("%s|%s\n", backend, renderer);
                return 0;
            }
            if (explicit_renderer) break;
        }
        if (explicit_backend) break;
    }
    fprintf(stderr, "No working OpenGL display path. Check your desktop/GPU setup or explicitly enable X11 software rendering.\n");
    return 1;
}
