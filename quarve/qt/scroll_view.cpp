#include "../inc/util.h"
#include "../inc/front.h"

extern int performing_subview_insertion;

extern "C" void *
back_view_scroll_init(
    uint8_t allow_vertical,
    uint8_t allow_horizontal,
    fat_pointer vertical_offset,
    fat_pointer horizontal_offset
)
{
    return nullptr;
}

extern "C" void
back_view_scroll_set_x(void *backing, double value)
{

}

extern "C" void
back_view_scroll_set_y(void *backing, double value)
{

}
