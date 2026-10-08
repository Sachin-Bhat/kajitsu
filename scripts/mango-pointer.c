/* Virtual pointer used only by the disposable Mango panel check. */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include <wayland-client.h>
#include "pointer-protocol.h"

struct output {
    struct wl_output *object;
    char name[64];
};
static struct output outputs[16];
static int output_count;
static struct wl_seat *seat;
static struct zwlr_virtual_pointer_manager_v1 *manager;

static void geometry(void *data, struct wl_output *output, int32_t x, int32_t y,
                     int32_t width, int32_t height, int32_t subpixel,
                     const char *make, const char *model, int32_t transform) {}
static void mode(void *data, struct wl_output *output, uint32_t flags,
                 int32_t width, int32_t height, int32_t refresh) {}
static void done(void *data, struct wl_output *output) {}
static void scale(void *data, struct wl_output *output, int32_t factor) {}
static void name(void *data, struct wl_output *output, const char *name) {
    snprintf(((struct output *)data)->name, 64, "%s", name);
}
static void description(void *data, struct wl_output *output, const char *text) {}
static const struct wl_output_listener output_listener = {
    geometry, mode, done, scale, name, description
};

static void global(void *data, struct wl_registry *registry, uint32_t id,
                   const char *interface, uint32_t version) {
    if (!strcmp(interface, "zwlr_virtual_pointer_manager_v1") && version >= 2)
        manager = wl_registry_bind(registry, id,
                    &zwlr_virtual_pointer_manager_v1_interface, 2);
    if (!strcmp(interface, "wl_seat"))
        seat = wl_registry_bind(registry, id, &wl_seat_interface, 1);
    if (!strcmp(interface, "wl_output") && version >= 4 && output_count < 16) {
        struct output *output = &outputs[output_count++];
        output->object = wl_registry_bind(registry, id, &wl_output_interface, 4);
        wl_output_add_listener(output->object, &output_listener, output);
    }
}
static void removed(void *data, struct wl_registry *registry, uint32_t id) {}
static const struct wl_registry_listener registry_listener = {global, removed};

int main(int argc, char **argv) {
    if (argc != 4) {
        fprintf(stderr, "usage: mango-pointer OUTPUT WIDTH HEIGHT (coordinates on stdin)\n");
        return 1;
    }
    struct wl_display *display = wl_display_connect(NULL);
    if (!display) return 2;
    struct wl_registry *registry = wl_display_get_registry(display);
    wl_registry_add_listener(registry, &registry_listener, NULL);
    if (wl_display_roundtrip(display) < 0 || wl_display_roundtrip(display) < 0)
        return 3;
    struct wl_output *output = NULL;
    for (int index = 0; index < output_count; index++)
        if (!strcmp(outputs[index].name, argv[1])) output = outputs[index].object;
    if (!manager || !seat || !output) return 4;
    struct zwlr_virtual_pointer_v1 *pointer =
        zwlr_virtual_pointer_manager_v1_create_virtual_pointer_with_output(manager, seat, output);
    wl_display_roundtrip(display);
    usleep(250000);
    unsigned x, y, tick = 1;
    int result = 0;
    while (scanf("%u %u", &x, &y) == 2) {
        zwlr_virtual_pointer_v1_motion_absolute(pointer, tick++, x, y,
                                              atoi(argv[2]), atoi(argv[3]));
        zwlr_virtual_pointer_v1_frame(pointer);
        wl_display_roundtrip(display);
        zwlr_virtual_pointer_v1_button(pointer, tick++, 272, WL_POINTER_BUTTON_STATE_PRESSED);
        zwlr_virtual_pointer_v1_frame(pointer);
        wl_display_roundtrip(display);
        zwlr_virtual_pointer_v1_button(pointer, tick++, 272, WL_POINTER_BUTTON_STATE_RELEASED);
        zwlr_virtual_pointer_v1_frame(pointer);
        result = wl_display_roundtrip(display);
        if (result < 0) break;
        puts("clicked");
        fflush(stdout);
    }
    wl_display_disconnect(display);
    return result < 0 ? 5 : 0;
}
