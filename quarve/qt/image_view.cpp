#include "../inc/util.h"

extern "C" void *
back_view_image_init(uint8_t const* path)
{
    return nullptr;
}

extern "C" size
back_view_image_size(void *_image)
{
    return (size) { 0, 0 };
}
