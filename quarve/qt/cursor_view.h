#pragma once

#include <QWidget>
#include <QHoverEvent>
#include <QCursor>

class CursorView : public QWidget {
public:
    Qt::CursorShape cursor;
    CursorView(Qt::CursorShape cursor) : cursor{cursor} {}

protected:
    void enterEvent(QEnterEvent *event) override {
        (void) event;
        setCursor(QCursor(cursor));
    }

    void leaveEvent(QEvent *event) override {
        (void) event;
        unsetCursor();
    }
};
