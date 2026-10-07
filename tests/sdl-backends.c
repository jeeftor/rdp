#include <SDL3/SDL.h>
#include <stdio.h>
#include <string.h>

/* Verify the packaged SDL library against an actual display server. */
int main(int argc, char **argv)
{
    if (argc != 2 || !SDL_Init(SDL_INIT_VIDEO)) {
        fprintf(stderr, "SDL initialization failed: %s\n", SDL_GetError());
        return 1;
    }
    const char *driver = SDL_GetCurrentVideoDriver();
    SDL_Window *window = SDL_CreateWindow("Backend smoke test", 320, 240, 0);
    int count = 0;
    SDL_DisplayID *displays = SDL_GetDisplays(&count);
    int failed = !driver || strcmp(driver, argv[1]) || !window || !displays || count < 1;
    printf("driver=%s displays=%d window=%s\n", driver ? driver : "none", count, window ? "created" : SDL_GetError());
    SDL_free(displays);
    SDL_DestroyWindow(window);
    SDL_Quit();
    return failed;
}
