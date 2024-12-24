#include <QtWidgets>
#include <vector>

#include "debug.h"
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
class Window : public QWidget {
public:
    Window() {}
    fat_pointer handle{};

protected:
    void changeEvent(QEvent *event) override {
        if (event->type() == QEvent::WindowStateChange) {
            QWindowStateChangeEvent *stateEvent = static_cast<QWindowStateChangeEvent*>(event);
            Qt::WindowStates oldState = stateEvent->oldState();
            Qt::WindowStates newState = windowState();

            if ((newState & Qt::WindowFullScreen) != (oldState & Qt::WindowFullScreen)) {
                front_window_will_fullscreen(this->handle, (newState & Qt::WindowFullScreen) != 0);
            }
        }
        QWidget::changeEvent(event);
    }

    inline void closeEvent(QCloseEvent *event) override
    {
        event->ignore();
        // a bit tight since we're freeing
        // at the same time it's being called??
        // however, doing it a frame
        // later actually does cause problems
        // when we do command q
        front_window_should_close(this->handle);
    }
};

extern "C" void *
back_window_init() {
    Window *window = new Window{};
    window->setAttribute(Qt::WA_DeleteOnClose, false);
    window->show();

    return window;
}

extern "C" void
back_window_set_handle(void *_window, fat_pointer handle) {
    Window *window = (Window*) _window;
    window->handle = handle;
}

extern "C" void
back_window_set_title(void *_window, uint8_t const* const title) {
    QWidget* window = (QWidget*) _window;
    window->setWindowTitle(QString::fromUtf8((char const*) title));
}

extern "C" void
back_window_set_needs_layout(void *_window) {

}

// should only be called once
extern "C" void
back_window_set_root(void *_window, void *root_view) {
    QWidget* widget = (QWidget*) _window;
    QWidget* root = (QWidget*) root_view;
    root->setParent(widget);
    root->show();
}

extern "C" void
back_window_set_size(void *_window, double w, double h) {
    QWidget* window = (QWidget*) _window;
    window->resize(w, h);
}

extern "C" void
back_window_set_min_size(void *_window, double w, double h) {
    QWidget* window = (QWidget*) _window;
    window->setMinimumSize(w, h);
}

extern "C" void
back_window_set_max_size(void *_window, double w, double h) {
    QWidget* window = (QWidget*) _window;
    window->setMaximumSize(w, h);
}

extern "C" void
back_window_set_fullscreen(void *_window, uint8_t fs) {
    QWidget* window = (QWidget*) _window;
    if (fs) {
        window->setWindowState(
            window->windowState() | Qt::WindowFullScreen
        );
    }
    else {
        window->setWindowState(
            window->windowState() & ~Qt::WindowFullScreen
        );
    }
}

extern "C" void
back_window_set_menu(void *_window, void *_menu)
{

}

extern "C" void
back_window_exit(void *window_p) {
    Window* window = (Window*) window_p;
    window->close();
}

extern "C" void
back_window_free(void *_window) {
    Window* window = (Window*) _window;
    while (QWidget* w = window->findChild<QWidget*>(Qt::FindDirectChildrenOnly)) {
        w->setParent(nullptr);
    }
    delete window;
}

/* view methods */
extern "C" void
back_view_clear_children(void *_view) {
    QWidget* view = (QWidget*) (_view);
    while (QWidget* w = view->findChild<QWidget*>(Qt::FindDirectChildrenOnly)) {
        w->setParent(nullptr);
    }
}

extern "C" void
back_view_remove_child(void *_view, unsigned long long index) {
    QWidget* view = (QWidget*) _view;
    const QObjectList& childList = view->children();
    QWidget* child = qobject_cast<QWidget*>(childList.at(index));
    child->setParent(nullptr);
}

extern "C" void
back_view_insert_child(void *_view, void* _child, unsigned long long index) {
    QWidget* view = (QWidget*) _view;
    const QObjectList& childList = view->children();

    // remove everything at the end
    std::vector<QWidget*> removed;
    for (int i = (int) childList.size() - 1; i >= (int) index; --i) {
        QWidget* old = qobject_cast<QWidget*>(childList.at(i));
        old->setParent(nullptr);
        removed.push_back(old);
    }

    QWidget* child = (QWidget*) _child;
    child->setParent(view);
    child->show();

    for (int i = removed.size() - 1; i >= 0; --i) {
        removed[i]->setParent(view);
    }
}

extern "C" void
back_view_set_frame(void *_view, double left, double top, double width, double height) {
    QWidget* view = (QWidget*) _view;

    int li = (int) left;
    int ti = (int) top;
    int wi = (int) width;
    int hi = (int) height;

    if (view->size().width() != wi || view->size().height() != hi) {
        view->resize(wi, hi);
    }

    if (view->pos().x() != li || view->pos().y() != ti) {
        view->move(li, ti);
        std::cerr << "Position " << li << " " << ti << std::endl;
    }
}

extern "C" void
back_free_view(void *_view) {
    back_view_clear_children(_view);

    QWidget* view = (QWidget*) _view;
    delete view;
}
