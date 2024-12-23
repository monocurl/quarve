#include <QtWidgets>
#include <iostream>

#include "color.h"
#include "../inc/util.h"
#include "front.h"

/* internal _state */
int performing_subview_insertion = 0;

/* global methods */
extern "C" void
back_main_loop() {
    int argc = 0;
    char* argv[] = {};
    QApplication a(argc, argv);
    front_will_spawn();
    a.exec();
}

extern "C" void
back_run_main(fat_pointer box) {
    QTimer::singleShot(0, [=]{
        front_execute_fn_once(box);
    });
}

extern "C" void
back_terminate() {
    QCoreApplication::instance()->quit();
}

/* window methods */

extern "C" void *
back_window_init() {
    QWidget *window = new QWidget{};
    window->show();
    return window;
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
    QWidget* widget = (QWidget*) _window;
    widget->resize(w, h);
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
    QWidget* window = (QWidget*) _window;
    delete window;
}

/* view methods */
extern "C" void
back_view_clear_children(void *_view) {
    QWidget* view = (QWidget*) (_view);
    while (QWidget* w = view->findChild<QWidget*>(Qt::FindDirectChildrenOnly)) {
        delete w;
    }
}

extern "C" void
back_view_remove_child(void *_view, unsigned long long index) {

}

extern "C" void
back_view_insert_child(void *_view, void* _child, unsigned long long index) {

}

extern "C" void
back_view_set_frame(void *_view, double left, double top, double width, double height) {
    QWidget* view = (QWidget*) _view;
    view->resize(width, height);
}

extern "C" void
back_free_view(void *_view) {
    QWidget* view = (QWidget*) _view;
    delete view;
}
