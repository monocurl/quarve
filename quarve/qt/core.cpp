#include <QtWidgets>
#include <QMenuBar>

#include <vector>
#include <cstring>

#include "debug.h"
#include "color.h"
#include "../inc/util.h"
#include "front.h"

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
class Window : public QMainWindow {
public:
    fat_pointer handle{};
    bool needsLayout{false};

    Window() { }

    void scheduleLayout() {
        if (needsLayout) {
            // already scheduled
            return;
        }

        this->needsLayout = true;

        QPointer<Window> safeThis = this;
        QTimer::singleShot(0, [safeThis]() {
            if (safeThis) {
                safeThis->layout();
            }
        });
    }

private:
    void layout() {
        if (!needsLayout || !this->handle.p0) {
            return;
        }

        front_window_layout(this->handle, this->width(), this->height());
        this->needsLayout = false;
    }

protected:
    void resizeEvent(QResizeEvent* event) override {
        QWidget::resizeEvent(event);
        this->needsLayout = true;
        this->layout();
    }

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

    void closeEvent(QCloseEvent *event) override
    {
        event->ignore();
        front_window_should_close(this->handle);
    }

    bool eventFilter(QObject *watched, QEvent *event) override {
        if (QWidget* widget = qobject_cast<QWidget*>(watched)) {
            buffer_event be = { .native_event = event };
            bool valid = false;

            // holds characters
            unsigned char buffer[64];

            if (event->type() == QEvent::KeyPress || event->type() == QEvent::KeyRelease) {
                valid = true;

                QKeyEvent* keyEvent = static_cast<QKeyEvent*>(event);

                if (keyEvent->modifiers() & Qt::ControlModifier) {
                    be.modifiers |= EVENT_MODIFIER_META;
                }
                if (keyEvent->modifiers() & Qt::ShiftModifier) {
                    be.modifiers |= EVENT_MODIFIER_SHIFT;
                }
                if (keyEvent->modifiers() & Qt::AltModifier) {
                    be.modifiers |= EVENT_MODIFIER_ALT_OPTION;
                }
                if (keyEvent->modifiers() & Qt::MetaModifier) {
                    be.modifiers |= EVENT_MODIFIER_CONTROL;
                }

                if (event->type() == QEvent::KeyPress && !keyEvent->isAutoRepeat()) {
                    be.is_down = true;
                } else if (event->type() == QEvent::KeyRelease) {
                    be.is_up = true;
                }

                strncpy((char *) buffer, keyEvent->text().toUtf8().data(), (sizeof buffer) - 1);
                buffer[(sizeof buffer) - 1] = '\0';
                be.key_characters = buffer;
            }
            else if (event->type() == QEvent::MouseButtonPress ||
                     event->type() == QEvent::MouseButtonRelease ||
                     event->type() == QEvent::MouseMove) {
                valid = true;

                QMouseEvent* mouseEvent = static_cast<QMouseEvent*>(event);
                be.is_mouse = true;

                if (event->type() == QEvent::MouseButtonPress) {
                    if (mouseEvent->button() == Qt::LeftButton) {
                        be.is_left_button = true;
                        be.is_down = true;
                    } else if (mouseEvent->button() == Qt::RightButton) {
                        be.is_right_button = true;
                        be.is_down = true;
                    }
                }
                else if (event->type() == QEvent::MouseButtonRelease) {
                    if (mouseEvent->button() == Qt::LeftButton) {
                        be.is_left_button = true;
                        be.is_up = true;
                    } else if (mouseEvent->button() == Qt::RightButton) {
                        be.is_right_button = true;
                        be.is_up = true;
                    }
                }

                be.cursor_x = mouseEvent->position().x();
                be.cursor_y = mouseEvent->position().y();
            }
            else if (event->type() == QEvent::Wheel) {
                valid = true;

                QWheelEvent* wheelEvent = static_cast<QWheelEvent*>(event);
                be.is_mouse = true;
                be.is_scroll = true;
                be.delta_x = wheelEvent->angleDelta().x();
                be.delta_y = wheelEvent->angleDelta().y();

                be.cursor_x = wheelEvent->position().x();
                be.cursor_y = wheelEvent->position().y();
            }

            if (valid) {
                return front_window_dispatch_event(this->handle, be) != 0;
            }
            // else fallthrough
        }

        return QObject::eventFilter(watched, event);
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
    window->installEventFilter(window);
    window->handle = handle;
}

extern "C" void
back_window_set_title(void *_window, uint8_t const* const title) {
    QWidget* window = (QWidget*) _window;
    window->setWindowTitle(QString::fromUtf8((char const*) title));
}

extern "C" void
back_window_set_needs_layout(void *_window) {
    Window *window = (Window*) _window;
    window->scheduleLayout();
}

// should only be called once
extern "C" void
back_window_set_root(void *_window, void *root_view) {
    Window* widget = (Window*) _window;

    QWidget* content = (QWidget*) root_view;
    content->setParent(widget);
    content->show();
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
    Window* window = (Window *) _window;
    QMenuBar* mb = (QMenuBar *) _menu;

    window->setMenuBar(mb);
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
    }
}

extern "C" void
back_free_view(void *_view) {
    back_view_clear_children(_view);

    QWidget* view = (QWidget*) _view;
    delete view;
}
