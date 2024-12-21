#import <Cocoa/Cocoa.h>
#import "util.h"

void *
back_view_image_init(uint8_t const* path)
{
    NSString* const nsPath = [NSString stringWithUTF8String:(char const*) path];
    NSImage* const image = [[NSImage alloc] initByReferencingFile:nsPath];
    if (!image || !image.valid) {
        return NULL;
    }

    NSImageView *const imageView = [[NSImageView alloc] init];
    imageView.image = image;
    imageView.imageScaling = NSImageScaleProportionallyUpOrDown;
    return imageView;
}

size
back_view_image_size(void *_image)
{
    NSImageView* const image = _image;
    NSSize const intrinsic = image.intrinsicContentSize;

    return (size) { (double) intrinsic.width, (double) intrinsic.height };
}
