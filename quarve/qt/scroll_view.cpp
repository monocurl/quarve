#include <QScrollArea>
#include <QScrollBar>
#include <QWidget>
#include <QWheelEvent>
#include <QLabel>
#include <QPalette>
#include <Qt>

#include "../inc/util.h"
#include "qt_util.h"
#include "front.h"
#include "debug.h"

class ScrollView : public QScrollArea {
public:
    ScrollView(fat_pointer h, fat_pointer v) :
        ignore_scroll(false),
        last_x(0),
        last_y(0),
        binding_x(h),
        binding_y(v) {

        setFrameShape(QFrame::NoFrame);
        setBackgroundRole(QPalette::NoRole);
        this->setStyleSheet("QScrollArea, QScrollArea > QWidget > .QWidget { background: transparent; }");

        connect(horizontalScrollBar(), &QScrollBar::valueChanged, this,
            [this](int) { handleScroll(); });
        connect(verticalScrollBar(), &QScrollBar::valueChanged, this,
            [this](int) { handleScroll(); });
    }

    void setScrollPosition(double x, double y) {
        ignore_scroll = true;

        if (horizontalScrollBar()->value() != x) {
            horizontalScrollBar()->setValue(x);
        }
        if (verticalScrollBar()->value() != y) {
            verticalScrollBar()->setValue(y);
        }

        ignore_scroll = false;
    }

    ~ScrollView() {
        front_free_screen_unit_binding(binding_x);
        front_free_screen_unit_binding(binding_y);
    }

private:
    void handleScroll() {
        if (ignore_scroll) return;

        double x = horizontalScrollBar()->value();
        double y = verticalScrollBar()->value();

        if (fabs(x - last_x) > EPSILON || fabs(y - last_y) > EPSILON) {
            last_x = x;
            last_y = y;
            front_set_screen_unit_binding(binding_x, x);
            front_set_screen_unit_binding(binding_y, y);
        }
    }

    bool ignore_scroll;
    double last_x;
    double last_y;
    fat_pointer binding_x;
    fat_pointer binding_y;
};

extern "C" void*
back_view_scroll_init(
    uint8_t allow_vertical,
    uint8_t allow_horizontal,
    fat_pointer vertical_offset,
    fat_pointer horizontal_offset
) {
    auto* scroll = new ScrollView(horizontal_offset, vertical_offset);

    scroll->setVerticalScrollBarPolicy(
        allow_vertical ? Qt::ScrollBarAsNeeded : Qt::ScrollBarAlwaysOff);
    scroll->setHorizontalScrollBarPolicy(
        allow_horizontal ? Qt::ScrollBarAsNeeded : Qt::ScrollBarAlwaysOff);

    return scroll;
}

extern "C" void
back_view_scroll_set_x(void* backing, double value) {
    ScrollView* scroll = static_cast<ScrollView*>(backing);
    scroll->setScrollPosition(value, scroll->verticalScrollBar()->value());
}

extern "C" void
back_view_scroll_set_y(void* backing, double value) {
    ScrollView* scroll = static_cast<ScrollView*>(backing);
    scroll->setScrollPosition(scroll->horizontalScrollBar()->value(), value);
}

extern "C" void *
back_view_scroll_content_init()
{
    QWidget* ret = new QWidget{};
    ret->setProperty(QUARVE_BACKEND_MOVED_PROPERTY, true);
    return ret;
}