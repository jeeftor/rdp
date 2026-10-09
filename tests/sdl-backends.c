#include <SDL3/SDL.h>
#include <stdio.h>
#include <string.h>

/* Verify the packaged SDL library against an actual display server. */
int main(int argc, char **argv)
{
    if ((argc != 2 && argc != 3) || !SDL_Init(SDL_INIT_VIDEO)) {
        fprintf(stderr, "SDL initialization failed: %s\n", SDL_GetError());
        return 1;
    }
    const char *driver = SDL_GetCurrentVideoDriver();
    SDL_Window *window = SDL_CreateWindow("Backend smoke test", 320, 240, 0);
    int count = 0;
    SDL_DisplayID *displays = SDL_GetDisplays(&count);
    int failed = !driver || strcmp(driver, argv[1]) || !window || !displays || count < 1;
    printf("driver=%s displays=%d window=%s\n", driver ? driver : "none", count, window ? "created" : SDL_GetError());
    if (argc == 3 && window) {
        SDL_Renderer *renderer = SDL_CreateRenderer(window, "software");
        SDL_Texture *target = renderer ? SDL_CreateTexture(renderer, SDL_PIXELFORMAT_BGRA32, SDL_TEXTUREACCESS_TARGET, 320, 240) : NULL;
        if (!target || !SDL_SetRenderTarget(renderer, target) || !SDL_SetRenderDrawColor(renderer, 0, 0, 0, 255)
            || !SDL_RenderClear(renderer) || !SDL_SetRenderTarget(renderer, NULL)
            || !SDL_RenderTexture(renderer, target, NULL, NULL) || !SDL_RenderPresent(renderer)) {
            fprintf(stderr, "Software desktop rendering failed: %s\n", SDL_GetError());
            failed = 1;
        } else {
            printf("software renderer: framebuffer and target texture presented\n");
        }
        SDL_DestroyTexture(target);
        SDL_DestroyRenderer(renderer);
    }
    SDL_free(displays);
    SDL_DestroyWindow(window);
    SDL_Quit();
    return failed;
}
