#include "../inc/util.h"

#include <QLabel>
#include <QPixmap>
#include <QImageReader>

class ImageView : public QLabel {

public:
    explicit ImageView(const QString& path)
    {
        setScaledContents(true);

        QImageReader reader(path);
        if (reader.canRead()) {
            originalPixmap = QPixmap(path);
            setPixmap(originalPixmap);

            setSizePolicy(QSizePolicy::Expanding, QSizePolicy::Expanding);
            setMinimumSize(1, 1);
        }
    }

    QSize sizeHint() const override
    {
        return originalPixmap.size();
    }

    QSize intrinsicSize() const
    {
        return originalPixmap.size();
    }

private:
    QPixmap originalPixmap;
};

extern "C" void*
back_view_image_init(uint8_t const* path)
{
    QString qPath = QString::fromUtf8(reinterpret_cast<const char*>(path));
    ImageView* view = new ImageView(qPath);

    if (view->pixmap().isNull()) {
        delete view;
        return nullptr;
    }

    return view;
}

extern "C" size
back_view_image_size(void* _image)
{
    ImageView* imageView = (ImageView*) _image;
    QSize intrinsic = imageView->intrinsicSize();
    return {
        static_cast<double>(intrinsic.width()),
        static_cast<double>(intrinsic.height())
    };
}
