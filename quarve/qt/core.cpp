#include "color.h"
#include "../inc/util.h"
#include "../inc/front.h"

/* internal _state */
int performing_subview_insertion = 0;

/* global methods */
extern "C" void
back_main_loop() {

}

extern "C" void
back_run_main(fat_pointer box) {
}

extern "C" void
back_terminate() {
}

/* window methods */

extern "C" void *
back_window_init() {
    return nullptr;
}

extern "C" void
back_window_set_handle(void *_window, fat_pointer handle) {

}

extern "C" void
back_window_set_title(void *_window, uint8_t const* const title) {
}

extern "C" void
back_window_set_needs_layout(void *_window) {

}

// should only be called once
extern "C" void
back_window_set_root(void *_window, void *root_view) {

}

extern "C" void
back_window_set_size(void *_window, double w, double h) {

}

extern "C" void
back_window_set_min_size(void *_window, double w, double h) {

}

extern "C" void
back_window_set_max_size(void *_window, double w, double h) {

}

extern "C" void
back_window_set_fullscreen(void *_window, uint8_t fs) {

}

extern "C" void
back_window_set_menu(void *_window, void *_menu)
{

}

extern "C" void
back_window_exit(void *window_p) {

}

extern "C" void
back_window_free(void *_window) {

}

/* view methods */
extern "C" void
back_view_clear_children(void *_view) {

}

extern "C" void
back_view_remove_child(void *_view, unsigned long long index) {

}

extern "C" void
back_view_insert_child(void *_view, void* _child, unsigned long long index) {

}

extern "C" void
back_view_set_frame(void *_view, double left, double top, double width, double height) {

}

extern "C" void
back_free_view(void *view) {

}
