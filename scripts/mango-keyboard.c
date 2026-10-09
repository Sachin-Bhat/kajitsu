/* Virtual keyboard for disposable Mango UI checks. */
#define _GNU_SOURCE
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include <sys/mman.h>
#include <wayland-client.h>
#include <xkbcommon/xkbcommon.h>
#include "keyboard-protocol.h"

static struct wl_seat *seat;
static struct zwp_virtual_keyboard_manager_v1 *manager;
static void global(void *data, struct wl_registry *registry, uint32_t id,
                   const char *interface, uint32_t version) {
    if (!strcmp(interface, "zwp_virtual_keyboard_manager_v1"))
        manager = wl_registry_bind(registry, id, &zwp_virtual_keyboard_manager_v1_interface, 1);
    if (!strcmp(interface, "wl_seat"))
        seat = wl_registry_bind(registry, id, &wl_seat_interface, 1);
}
static void removed(void *data, struct wl_registry *registry, uint32_t id) {}
static const struct wl_registry_listener listener = {global, removed};
int main(void) {
    struct wl_display *display = wl_display_connect(NULL);
    if (!display) return 2;
    wl_registry_add_listener(wl_display_get_registry(display), &listener, NULL);
    if (wl_display_roundtrip(display) < 0 || !seat || !manager) return 3;
    struct zwp_virtual_keyboard_v1 *keyboard = zwp_virtual_keyboard_manager_v1_create_virtual_keyboard(manager, seat);
    struct xkb_context *context = xkb_context_new(XKB_CONTEXT_NO_FLAGS);
    struct xkb_keymap *keymap = xkb_keymap_new_from_names(context, NULL, XKB_KEYMAP_COMPILE_NO_FLAGS);
    if (!keymap) return 4;
    char *text = xkb_keymap_get_as_string(keymap, XKB_KEYMAP_FORMAT_TEXT_V1);
    if (!text) return 4;
    size_t length = strlen(text) + 1;
    int fd = memfd_create("kajitsu-test-keymap", MFD_CLOEXEC);
    if (fd < 0 || write(fd, text, length) != (ssize_t)length) return 4;
    zwp_virtual_keyboard_v1_keymap(keyboard, WL_KEYBOARD_KEYMAP_FORMAT_XKB_V1, fd, length);
    wl_display_roundtrip(display);
    close(fd); free(text); xkb_keymap_unref(keymap); xkb_context_unref(context);
    const char *names[] = {"up", "down", "left", "right", "enter", "space", "home", "end", "escape"};
    unsigned codes[] = {103, 108, 105, 106, 28, 57, 102, 107, 1};
    char line[64]; unsigned tick = 1;
    while (fgets(line, sizeof(line), stdin)) {
        line[strcspn(line, "\r\n")] = 0;
        for (int i = 0; i < 9; i++) {
            if (strcmp(line, names[i])) continue;
            zwp_virtual_keyboard_v1_key(keyboard, tick++, codes[i], WL_KEYBOARD_KEY_STATE_PRESSED);
            wl_display_roundtrip(display);
            zwp_virtual_keyboard_v1_key(keyboard, tick++, codes[i], WL_KEYBOARD_KEY_STATE_RELEASED);
            if (wl_display_roundtrip(display) < 0) return 5;
            puts("keyed"); fflush(stdout); break;
        }
    }
    wl_display_disconnect(display);
    return 0;
}
