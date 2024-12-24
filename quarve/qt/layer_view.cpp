#include <QtWidgets>
#include "debug.h"
#include "color.h"
#include "../inc/util.h"

class CustomWidget : public QWidget {
public:
    void setBackgroundColor(const QColor& color) {
        backgroundColor = color;
    }

    void setBorderColor(const QColor& color) {
        borderColor = color;
    }

    void setBorderWidth(int width) {
        borderWidth = width;
    }

    void setCornerRadius(int radius) {
        cornerRadius = radius;
    }

    void setOpacity(double opacity) {
        if ((1 - opacity) > EPSILON) {
            QGraphicsOpacityEffect* effect = new QGraphicsOpacityEffect(this);
            effect->setOpacity(opacity);
            this->setGraphicsEffect(effect);
        }
        else {
            this->setGraphicsEffect(nullptr);
        }
    }

protected:
    void paintEvent(QPaintEvent* event) override {
        QWidget::paintEvent(event);

        QPainter painter(this);

        // background
        QColor bgColor = backgroundColor;
        painter.setBrush(bgColor);
        painter.setPen(Qt::NoPen);
        painter.setRenderHint(QPainter::Antialiasing);
        QRectF rect = this->rect().adjusted(borderWidth / 2.0, borderWidth / 2.0, -borderWidth / 2.0, -borderWidth / 2.0);
        painter.drawRoundedRect(rect, cornerRadius, cornerRadius);

        // border
        QPen pen(borderColor);
        pen.setWidth(borderWidth);
        painter.setPen(pen);
        painter.setBrush(Qt::NoBrush);
        painter.drawRoundedRect(rect, cornerRadius, cornerRadius);
    }

private:
    QColor backgroundColor = Qt::white;
    QColor borderColor = Qt::black;
    int borderWidth = 1;
    int cornerRadius = 0;
};

extern "C" void *
back_view_layer_init()
{
    return new CustomWidget{};
}

extern "C" void
back_view_layer_update(void *_view, color background_color, color border_color, double corner_radius, double border_width, float opacity)
{
    CustomWidget* view = (CustomWidget*) _view;

    view->setBackgroundColor(QColor(
        background_color.r,
        background_color.g,
        background_color.b,
        background_color.a
    ));
    view->setBorderColor(QColor(
        border_color.r,
        border_color.g,
        border_color.b,
        border_color.a
    ));

    view->setBorderWidth(static_cast<int>(border_width));
    view->setCornerRadius(static_cast<int>(corner_radius));
    view->setOpacity(static_cast<double>(opacity));

    view->update();
}
