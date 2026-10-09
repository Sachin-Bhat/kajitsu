/* Toggle only named disposable outputs through output-management v1. */
#include <stdio.h>
#include <stdlib.h>
#include <math.h>
#include <string.h>
#include <wayland-client.h>
#include "outputs-protocol.h"
struct head { struct zwlr_output_head_v1 *object; struct zwlr_output_mode_v1 *mode; char name[64]; int enabled, x, y, transform; wl_fixed_t scale; };
static struct head heads[16];
static int count, result;
static uint32_t serial;
static struct zwlr_output_manager_v1 *manager;
static void mode_size(void *d, struct zwlr_output_mode_v1 *m, int32_t w, int32_t h) {}
static void mode_refresh(void *d, struct zwlr_output_mode_v1 *m, int32_t r) {}
static void mode_preferred(void *d, struct zwlr_output_mode_v1 *m) {}
static void mode_finished(void *d, struct zwlr_output_mode_v1 *m) {}
static const struct zwlr_output_mode_v1_listener mode_listener = {mode_size, mode_refresh, mode_preferred, mode_finished};
static void name(void *d, struct zwlr_output_head_v1 *h, const char *s) { snprintf(((struct head *)d)->name, 64, "%s", s); }
static void text(void *d, struct zwlr_output_head_v1 *h, const char *s) {}
static void physical(void *d, struct zwlr_output_head_v1 *h, int32_t w, int32_t v) {}
static void mode(void *d, struct zwlr_output_head_v1 *h, struct zwlr_output_mode_v1 *m) { struct head *head = d; if (!head->mode) head->mode = m; zwlr_output_mode_v1_add_listener(m, &mode_listener, NULL); }
static void enabled(void *d, struct zwlr_output_head_v1 *h, int32_t value) { ((struct head *)d)->enabled = value; }
static void current(void *d, struct zwlr_output_head_v1 *h, struct zwlr_output_mode_v1 *m) { ((struct head *)d)->mode = m; }
static void position(void *d, struct zwlr_output_head_v1 *h, int32_t x, int32_t y) { ((struct head *)d)->x = x; ((struct head *)d)->y = y; }
static void transform(void *d, struct zwlr_output_head_v1 *h, int32_t value) { ((struct head *)d)->transform = value; }
static void scale(void *d, struct zwlr_output_head_v1 *h, wl_fixed_t value) { ((struct head *)d)->scale = value; }
static void finished(void *d, struct zwlr_output_head_v1 *h) { ((struct head *)d)->object = NULL; }
static const struct zwlr_output_head_v1_listener head_listener = {.name=name, .description=text, .physical_size=physical, .mode=mode, .enabled=enabled, .current_mode=current, .position=position, .transform=transform, .scale=scale, .finished=finished};
static void head(void *d, struct zwlr_output_manager_v1 *m, struct zwlr_output_head_v1 *h) { if (count >= 16) return; struct head *s = &heads[count++]; s->object = h; s->scale = wl_fixed_from_int(1); zwlr_output_head_v1_add_listener(h, &head_listener, s); }
static void done(void *d, struct zwlr_output_manager_v1 *m, uint32_t s) { serial=s; }
static void stopped(void *d, struct zwlr_output_manager_v1 *m) {}
static const struct zwlr_output_manager_v1_listener manager_listener = {head, done, stopped};
static void global(void *d, struct wl_registry *r, uint32_t id, const char *name, uint32_t version) { if (!strcmp(name, "zwlr_output_manager_v1")) { manager=wl_registry_bind(r,id,&zwlr_output_manager_v1_interface,1); zwlr_output_manager_v1_add_listener(manager,&manager_listener,NULL); } }
static void removed(void *d, struct wl_registry *r, uint32_t id) {}
static const struct wl_registry_listener registry_listener = {global, removed};
static void success(void *d, struct zwlr_output_configuration_v1 *c) { result=1; }
static void failure(void *d, struct zwlr_output_configuration_v1 *c) { result=-1; }
static const struct zwlr_output_configuration_v1_listener config_listener = {success, failure, failure};
int main(int argc, char **argv) {
    if ((argc != 3 && argc != 4) || strncmp(argv[1], "HEADLESS-", 9) || (strcmp(argv[2], "on") && strcmp(argv[2], "off"))) return 1;
    double restored_scale = argc == 4 ? atof(argv[3]) : 1.0;
    if (!isfinite(restored_scale) || restored_scale <= 0 || restored_scale > 4) return 1;
    struct wl_display *display=wl_display_connect(NULL); if (!display) return 2;
    wl_registry_add_listener(wl_display_get_registry(display),&registry_listener,NULL);
    if (wl_display_roundtrip(display)<0 || wl_display_roundtrip(display)<0 || !manager) return 3;
    int found=0; struct zwlr_output_configuration_v1 *config=zwlr_output_manager_v1_create_configuration(manager,serial);
    zwlr_output_configuration_v1_add_listener(config,&config_listener,NULL);
    for (int i=0;i<count;i++) {
        struct head *h=&heads[i]; if (!h->object) continue;
        int target=!strcmp(h->name,argv[1]); if (target) found=1;
        int enable=target ? !strcmp(argv[2],"on") : h->enabled;
        if (!enable) { zwlr_output_configuration_v1_disable_head(config,h->object); continue; }
        struct zwlr_output_configuration_head_v1 *c=zwlr_output_configuration_v1_enable_head(config,h->object);
        if (h->mode) zwlr_output_configuration_head_v1_set_mode(c,h->mode);
        zwlr_output_configuration_head_v1_set_position(c,h->x,h->y);
        zwlr_output_configuration_head_v1_set_scale(c,target && argc == 4 ? wl_fixed_from_double(restored_scale) : h->scale);
        zwlr_output_configuration_head_v1_set_transform(c,h->transform);
    }
    if (!found) return 4;
    zwlr_output_configuration_v1_apply(config);
    while (!result && wl_display_dispatch(display)>=0) {}
    zwlr_output_configuration_v1_destroy(config); wl_display_disconnect(display);
    return result==1 ? 0 : 5;
}
