#pragma once
#import <Cocoa/Cocoa.h>

typedef struct color {
    uint8_t r, g, b, a;
} color;

static CGColorSpaceRef CSPACE_RGB;

// caller must free this
static inline CGColorRef
color_to_cg_color(color c)
{
    if (!CSPACE_RGB) {
        CSPACE_RGB = CGColorSpaceCreateDeviceRGB();
    }
    CGFloat comps[] = {c.r / 255.0, c.g / 255.0, c.b / 255.0, c.a / 255.0};
    return CGColorCreate(CSPACE_RGB, comps);
}
