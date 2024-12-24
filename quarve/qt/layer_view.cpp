#include <QtWidgets>
#include "color.h"
#include "../inc/util.h"

extern "C" void *
back_view_layer_init()
{
    return new QWidget{};
}

extern "C" void
back_view_layer_update(void *_view, color background_color, color border_color, double corner_radius, double border_width, float opacity)
{
    QWidget* view = (QWidget*) _view;

    QString bg_color = QString::fromUtf8("rgba(%1, %2, %3, %4)")
        .arg(background_color.r)
        .arg(background_color.g)
        .arg(background_color.b)
        .arg(background_color.a);

    QString brd_color = QString::fromUtf8("rgba(%1, %2, %3, %4)")
        .arg(border_color.r)
        .arg(border_color.g)
        .arg(border_color.b)
        .arg(border_color.a);

    QString style_sheet = QString::fromUtf8(
        "QWidget {"
        "  background-color: %1;"
        "  border: %2px solid %3;"
        "  border-radius: %4px;"
        "  opacity: %5;"
        "}"
    )
     .arg(bg_color)
     .arg(border_width)
     .arg(brd_color)
     .arg(corner_radius)
     .arg(opacity);

    view->setStyleSheet(style_sheet);
    view->setWindowOpacity(opacity);
    view->update();
}
