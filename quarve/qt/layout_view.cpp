#include <QWidget>
#include "../inc/util.h"

extern "C" void *
back_view_layout_init() {
    QWidget* ret = new QWidget{};
    return ret;
}
