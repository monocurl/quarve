#include <QWidget>
#include <QEvent>
#include <QMouseEvent>

#include "../inc/util.h"
#include "debug.h"

class LayoutView : public QWidget {
public:
    LayoutView() {

    }

protected:
    void enterEvent(QEnterEvent *event) override {
        event->ignore();
    }

    void leaveEvent(QEvent *event) override {
        event->ignore();
    }

    void mouseDoubleClickEvent(QMouseEvent *event) override {
        event->ignore();
    }

    void mousePressEvent(QMouseEvent *event) override {
        event->ignore();
    }

    void mouseReleaseEvent(QMouseEvent *event) override {
        event->ignore();
    }

    void mouseMoveEvent(QMouseEvent *event) override {
        event->ignore();
    }
};

extern "C" void *
back_view_layout_init() {
    LayoutView* ret = new LayoutView{};
    return ret;
}
