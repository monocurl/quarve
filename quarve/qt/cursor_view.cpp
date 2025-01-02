#include <QGuiApplication>

#include "cursor_view.h"
#include "../inc/util.h"

#include <cassert>

static Qt::CursorShape
fromQuarveCursor(int cursor_type)
{
    switch (cursor_type) {
        case CURSOR_ARROW:
            return Qt::ArrowCursor;
        case CURSOR_IBEAM:
            return Qt::IBeamCursor;
        case CURSOR_POINTER:
            return Qt::PointingHandCursor;
        case CURSOR_HORIZONTAL_RESIZE:
            return Qt::SplitHCursor;
        case CURSOR_VERTICAL_RESIZE:
            return Qt::SplitVCursor;
        default:
            assert(0);
            return Qt::ArrowCursor;
    }
}

extern "C" void *
back_view_cursor_init(int cursor_type)
{
    return new CursorView(fromQuarveCursor(cursor_type));
}

extern "C" void
back_view_cursor_update(void* _view, int cursor_type)
{
    CursorView* view = static_cast<CursorView*>(_view);
    view->cursor = fromQuarveCursor(cursor_type);
}

extern "C" void
back_push_cursor(int cursor) {
    Qt::CursorShape c = fromQuarveCursor(cursor);
    QGuiApplication::setOverrideCursor(QCursor(c));
}

extern "C" void
back_pop_cursor() {
    QGuiApplication::restoreOverrideCursor();
}
