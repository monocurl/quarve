#include <QtWidgets>

#include "../inc/util.h"
#include "cursor_view.h"

extern "C" void *
back_view_button_init() {
    return new CursorView(Qt::PointingHandCursor);
}

extern "C" void
back_view_button_update(void *_view, uint8_t clicked) {
    CursorView* cv = (CursorView*) _view;
    if (clicked) {
        QGraphicsOpacityEffect* effect = new QGraphicsOpacityEffect(cv);
        effect->setOpacity(0.7);
        cv->setGraphicsEffect(effect);
    }
    else {
        cv->setGraphicsEffect(nullptr);
    }
}
