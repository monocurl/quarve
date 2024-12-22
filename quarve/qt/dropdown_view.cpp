#include "../inc/util.h"
#include "../inc/front.h"


extern "C" void*
back_view_dropdown_init(fat_pointer binding) {
    return nullptr;
}

extern "C" void
back_view_dropdown_add(void *_view, unsigned char const* option) {

}

extern "C" uint8_t
back_view_dropdown_select(void *_view, unsigned char const* selection) {
    return 0;
}

extern "C" void
back_view_dropdown_clear(void* _view)
{

}

extern "C" size
back_view_dropdown_size(void *_view) {
    return (size) { 0, 0 };
}
