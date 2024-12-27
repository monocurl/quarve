#include <QLabel>
#include <QFontDatabase>
#include <QHash>
#include <QFile>

#include "../inc/util.h"
#include "color.h"
#include "debug.h"
#include "front.h"

static QHash<QString, QFont> fontCache;

// TODO probably not the best way to do this for Qt
static QString
createFontCacheKey(const QString& fontPath, double size, bool bold, bool italic) {
    return QString("%1;:;-%2-%3-%4").arg(fontPath).arg(size).arg(bold).arg(italic);
}

static QFont
getFont(const uint8_t* fontPath, double size, bool bold, bool italic) {
    QString path = fontPath ? QString::fromUtf8(reinterpret_cast<const char*>(fontPath)) : QString();
    QString cacheKey = createFontCacheKey(path, size, bold, italic);

    if (fontCache.contains(cacheKey)) {
        return fontCache[cacheKey];
    }

    QFont font;
    if (fontPath) {
        // Load font directly from file
        QFile fontFile(path);
        if (fontFile.open(QIODevice::ReadOnly)) {
            int id = QFontDatabase::addApplicationFontFromData(fontFile.readAll());
            if (id != -1) {
                QString family = QFontDatabase::applicationFontFamilies(id).at(0);
                font = QFont(family);
            } else {
                fprintf(stderr, "quarve: unable to load font %s; defaulting to system\n", fontPath);
                font = QFont();
            }
            fontFile.close();
        } else {
            fprintf(stderr, "quarve: unable to open font file %s; defaulting to system\n", fontPath);
            font = QFont();
        }
    } else {
        font = QFont();
    }

    font.setPointSizeF(size);
    font.setBold(bold);
    font.setItalic(italic);

    fontCache[cacheKey] = font;
    return font;
}

extern "C" void*
back_text_init()
{
    QLabel* label = new QLabel();
    label->setTextInteractionFlags(Qt::NoTextInteraction);
    label->setWordWrap(true);
    return label;
}

extern "C" void
back_text_update(
    void *view,
    uint8_t const* str,
    int max_lines,
    uint8_t bold,
    uint8_t italic,
    uint8_t underline,
    uint8_t strikethrough,
    color back,
    color front,
    uint8_t const* font_path,
    double font_size
)
{
    QLabel* label = static_cast<QLabel*>(view);

    QFont font = getFont(font_path, font_size, bold, italic);
    label->setFont(font);

    label->setTextFormat(Qt::PlainText);
    label->setText(QString::fromUtf8(reinterpret_cast<const char*>(str)));

    // set stylesheet
    QString style = QString("QLabel { color: rgba(%1, %2, %3, %4); ").arg(
        front.r).arg(front.g).arg(front.b).arg(front.a);

    if (back.a > 0) {
        style += QString("background-color: rgba(%1, %2, %3, %4); ").arg(
            back.r).arg(back.g).arg(back.b).arg(back.a);
    }

    if (underline && strikethrough) {
        style += "text-decoration: underline line-through; ";
    }
    else if (underline) {
        style += "text-decoration: underline; ";
    }
    else if (strikethrough) {
        style += "text-decoration: line-through; ";
    }

    style += "}";
    label->setStyleSheet(style);

    if (max_lines > 0) {
        label->setMaximumHeight(label->fontMetrics().height() * max_lines);
    } else {
        label->setMaximumHeight(QWIDGETSIZE_MAX);
    }
}

extern "C" size
back_text_size(void* view, size suggested)
{
    QLabel* label = static_cast<QLabel*>(view);

    QSize hint = label->fontMetrics().boundingRect(
        QRect(0, 0, suggested.w, 0),
        Qt::TextWordWrap | Qt::AlignLeft | Qt::AlignTop,
        label->text()
    ).size();

    return {static_cast<double>(hint.width()),
            static_cast<double>(hint.height())};
}

// MARK: textfield

extern "C" void*
back_text_field_init(
    fat_pointer text_binding,
    fat_pointer focused_binding,
    fat_pointer callback,
    int32_t token,
    uint8_t unstyled,
    uint8_t secure
)
{
    return nullptr;
}

extern "C" void
back_text_field_focus(void *view)
{

}

extern "C" void
back_text_field_unfocus(void *view)
{

}

extern "C" void
back_text_field_update(
    void *view,
    uint8_t const* str,
    int max_lines,
    uint8_t bold,
    uint8_t italic,
    uint8_t underline,
    uint8_t strikethrough,
    color back,
    color front,
    uint8_t const* font_path,
    double font_size
)
{

}

extern "C" size
back_text_field_size(void* view, size suggested)
{
    return (size) { 0, 0 };
}

extern "C" void
back_text_field_select_all(void *view)
{

}

extern "C" void
back_text_field_cut(void *view)
{

}

extern "C" void
back_text_field_copy(void *view)
{

}

extern "C" void
back_text_field_paste(void *view)
{

}

// MARK: textview
extern "C" void *
back_text_view_init()
{
    return nullptr;
}

// may discard attributes
extern "C" void
back_text_view_full_replace(
    void *tv,
    const uint8_t* with,
    fat_pointer _state,
    fat_pointer selected,
    fat_pointer key_callback
)
{

}

extern "C" void
back_text_view_replace(void *tv, size_t start, size_t len, const uint8_t* with)
{

}

extern "C" void
back_text_view_set_font(
    void *tv, uint8_t const* font_path, double font_size
)
{

}

extern "C" void
back_text_view_set_editing_state(void *tv, uint8_t editing)
{

}

extern "C" void
back_text_view_set_line_attributes(
    void *tv,
    size_t line_no, size_t start, size_t end,
    int justification_sign,
    double leading_indentation, double trailing_indentation
)
{

}

extern "C" void
back_text_view_set_char_attributes(
    void *tv, size_t start, size_t end,
    uint8_t bold, uint8_t italic, uint8_t underline, uint8_t strikethrough,
    color back, color front
)
{

}

extern "C" void
back_text_view_set_selection(void *tv, size_t start, size_t len)
{

}

extern "C" void
back_text_view_get_selection(void *tv, size_t *start, size_t* end)
{

}

extern "C" double
back_text_view_get_line_height(void *tv, size_t line, size_t start, size_t end, double width)
{
    return 0.0;
}

extern "C" void
back_text_view_get_cursor_pos(void *tv, double *x, double *y)
{

}


extern "C" void
back_text_view_set_page_id(void *tv, int32_t page_id)
{

}

extern "C" void
back_text_view_focus(void *tv)
{

}

extern "C" void
back_text_view_unfocus(void *tv)
{

}

extern "C" void
back_text_view_copy(void *tv)
{

}

extern "C" void
back_text_view_cut(void *tv)
{

}

extern "C" void
back_text_view_paste(void *tv)
{

}

extern "C" void
back_text_view_select_all(void *tv)
{

}