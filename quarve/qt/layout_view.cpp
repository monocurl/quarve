#include <QWidget>
#include "../inc/util.h"
#include "debug.h"

extern "C" void *
back_view_layout_init() {
    QWidget* ret = new QWidget{};
    return ret;
}
