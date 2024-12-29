#pragma once

#include <QWidget>
#include <QCursor>

class CursorView : public QWidget {
public:
    Qt::CursorShape cursor;
    CursorView(Qt::CursorShape cursor) : cursor{cursor} {
        setCursor(QCursor(cursor));
    }
};
