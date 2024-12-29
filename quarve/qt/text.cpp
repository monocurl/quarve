#include <QLabel>
#include <QFontDatabase>
#include <QHash>
#include <QFile>
#include <QTextEdit>
#include <QFocusEvent>
#include <QKeyEvent>
#include <QColor>
#include <QTextCursor>
#include <QTextCharFormat>
#include <QTextBlockFormat>
#include <QFontMetrics>
#include <QApplication>
#include <QClipboard>
#include <QTextBlock>

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

    double max_height = max_lines == 0 ?
        QWIDGETSIZE_MAX :
        label->fontMetrics().height() * max_lines;
    label->setMaximumHeight(max_height);
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
            static_cast<double>(std::min(hint.height(), label->maximumHeight()))};
}

// MARK: textfield

class TextField : public QTextEdit {
public:
    fat_pointer focused;
    fat_pointer text;
    fat_pointer callback;
    int32_t focused_token;
    bool scheduled_focused;
    bool executing_back;

    TextField() : scheduled_focused(false), executing_back(false) {
        connect(this, &QTextEdit::textChanged, [this]() {
            if (!this->executing_back) {
                front_set_opt_string_binding(this->text, (uint8_t const*)toPlainText().toUtf8().constData());
                front_execute_fn_mut(callback);
            }
        });

        setSizePolicy(QSizePolicy::Expanding, QSizePolicy::Expanding);
        setVerticalScrollBarPolicy(Qt::ScrollBarAlwaysOff);
        setHorizontalScrollBarPolicy(Qt::ScrollBarAlwaysOff);
    }

    ~TextField() {
        front_free_token_binding(focused);
        front_free_opt_string_binding(text);
        front_free_fn_mut(callback);
    }

    // https://stackoverflow.com/a/46997337
    void setHeight (int nRows)
    {
        if (nRows) {
            QTextDocument *pdoc = this->document();
            QFontMetrics fm(pdoc->defaultFont());
            QMargins margins = this->contentsMargins();
            int nHeight = fm.lineSpacing() * nRows +
                (pdoc->documentMargin() + this->frameWidth()) * 2 +
                margins.top() + margins.bottom();
            this->setMaximumHeight(nHeight);
        }
        else {
            this->setMaximumHeight(QWIDGETSIZE_MAX);
        }
    }

protected:
    void focusInEvent(QFocusEvent* event) override {
        QTextEdit::focusInEvent(event);
        front_set_token_binding(focused, 1, focused_token);
    }

    void focusOutEvent(QFocusEvent* event) override {
        QTextEdit::focusOutEvent(event);
        front_set_token_binding(focused, 0, focused_token);
    }

    void keyPressEvent(QKeyEvent* event) override {
        if (event->key() == Qt::Key_Escape) {
            clearFocus();
        } else if (event->key() == Qt::Key_Tab) {
            clearFocus();
            front_set_token_binding(focused, 1, focused_token + 1);
            event->accept();
        } else if (event->key() == Qt::Key_Backtab) {
            clearFocus();
            front_set_token_binding(focused, 1, focused_token - 1);
            event->accept();
        } else {
            QTextEdit::keyPressEvent(event);
        }
    }
};

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
    (void) secure;
    (void) unstyled;

    TextField* field = new TextField();
    field->focused = focused_binding;
    field->text = text_binding;
    field->callback = callback;
    field->focused_token = token;
    field->scheduled_focused = false;

    field->setFrameStyle(QFrame::NoFrame);

    return field;
}

extern "C" void
back_text_field_focus(void *view)
{
    TextField* field = static_cast<TextField*>(view);
    field->scheduled_focused = true;
    if (!field->hasFocus()) {
        field->setFocus();
    }
}

extern "C" void
back_text_field_unfocus(void *view)
{
    TextField* field = static_cast<TextField*>(view);
    field->scheduled_focused = false;
    if (field->hasFocus()) {
        field->clearFocus();
    }
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
    TextField* field = static_cast<TextField*>(view);

    QFont font = getFont(font_path, font_size, bold, italic);
    field->setFont(font);

    QString newText = QString::fromUtf8(reinterpret_cast<const char*>(str));
    if (field->toPlainText() != newText) {
        field->executing_back = true;
        field->setPlainText(newText);
        field->executing_back = false;
    }

    QString style = QString("QTextEdit { color: rgba(%1, %2, %3, %4); ")
        .arg(front.r).arg(front.g).arg(front.b).arg(front.a);

    if (back.a > 0) {
        style += QString("background: rgba(%1, %2, %3, %4); border: none; ")
            .arg(back.r).arg(back.g).arg(back.b).arg(back.a);
    }
    else {
        style += "background: transparent; border: none; ";
    }

    if (underline) {
        style += "text-decoration: underline; ";
    }
    if (strikethrough) {
        style += "text-decoration: line-through; ";
    }
    if (underline && strikethrough) {
        style += "text-decoration: underline line-through; ";
    }

    style += "}";

    if (style != field->styleSheet()) {
        field->setStyleSheet(style);
    }

    // Handle line limiting
    field->setHeight(max_lines);
}

extern "C" size
back_text_field_size(void* view, size suggested)
{
    TextField* field = static_cast<TextField*>(view);

    QSize hint = field->fontMetrics().boundingRect(
        QRect(0, 0, suggested.w, 0),
        Qt::TextWordWrap | Qt::AlignLeft | Qt::AlignTop,
        field->toPlainText()
    ).size();

    QTextDocument *pdoc = field->document();
    QFontMetrics fm(pdoc->defaultFont());
    QMargins margins = field->contentsMargins();
    int height = hint.height() +
        (pdoc->documentMargin() + field->frameWidth()) * 2 +
        margins.top() + margins.bottom();

    return { static_cast<double>(hint.width()),
             static_cast<double>(std::min(height, field->maximumHeight())) };
}

extern "C" void
back_text_field_select_all(void *view)
{
    TextField* field = static_cast<TextField*>(view);
    field->selectAll();
}

extern "C" void
back_text_field_cut(void *view)
{
    TextField* field = static_cast<TextField*>(view);
    field->cut();
}

extern "C" void
back_text_field_copy(void *view)
{
    TextField* field = static_cast<TextField*>(view);
    field->copy();
}

extern "C" void
back_text_field_paste(void *view)
{
    TextField* field = static_cast<TextField*>(view);
    field->paste();
}

// MARK: textview
class TextView : public QTextEdit {
public:
    TextView() : QTextEdit(), executing_back(false), page_id(0) {
        setFrameStyle(QFrame::NoFrame);
        setVerticalScrollBarPolicy(Qt::ScrollBarAlwaysOff);
        setHorizontalScrollBarPolicy(Qt::ScrollBarAlwaysOff);
        setContentsMargins(0, 0, 0, 0);
        document()->setDocumentMargin(0);

        setAcceptRichText(false);
        setUndoRedoEnabled(false);
        setAutoFormatting(QTextEdit::AutoNone);

        // Connect text change handler
        connect(document(), &QTextDocument::contentsChange, this,
            [this](int position, int removed, int added) {
                std::cerr << "Executed Contents Change " << std::endl;
                if (!executing_back) {
                    QString addedText = document()->toPlainText().mid(position, added);
                    front_replace_textview_range(text_view_state, position, removed,
                        reinterpret_cast<const uint8_t*>(addedText.toUtf8().constData()));
                }
            });

        // Connect selection change handler
        connect(this, &QTextEdit::selectionChanged, this,
            [this]() {
                std::cerr << "Executed Selection Change " << std::endl;
                if (!executing_back) {
                    QTextCursor cursor = textCursor();
                    front_set_textview_selection(text_view_state,
                        cursor.selectionStart(),
                        cursor.selectionEnd() - cursor.selectionStart());
                }
            });

        installEventFilter(this);
    }

    bool eventFilter(QObject *obj, QEvent *event) override {
        if (event->type() == QEvent::KeyPress) {
            QKeyEvent *keyEvent = static_cast<QKeyEvent *>(event);

            if (keyEvent->key() == Qt::Key_Escape) {
                if (front_execute_key_callback(key_handler, TEXTVIEW_CALLBACK_KEYCODE_ESCAPE)) {
                    return true;
                }
                clearFocus();
                return true;
            }
            else if (keyEvent->key() == Qt::Key_Tab) {
                if (front_execute_key_callback(key_handler, TEXTVIEW_CALLBACK_KEYCODE_TAB)) {
                    return true;
                }
            }
            else if (keyEvent->key() == Qt::Key_Backtab) {
                if (front_execute_key_callback(key_handler, TEXTVIEW_CALLBACK_KEYCODE_UNTAB)) {
                    return true;
                }
            }
            else if (keyEvent->key() == Qt::Key_Return || keyEvent->key() == Qt::Key_Enter) {
                if (keyEvent->modifiers() & Qt::ShiftModifier) {
                    if (front_execute_key_callback(key_handler, TEXTVIEW_CALLBACK_KEYCODE_NEWLINE)) {
                        return true;
                    }
                } else {
                    if (front_execute_key_callback(key_handler, TEXTVIEW_CALLBACK_KEYCODE_ALT_NEWLINE)) {
                        return true;
                    }
                }
            }
            else if (keyEvent->key() == Qt::Key_Up) {
                if (front_execute_key_callback(key_handler, TEXTVIEW_CALLBACK_KEYCODE_UP)) {
                    return true;
                }
            }
            else if (keyEvent->key() == Qt::Key_Down) {
                if (front_execute_key_callback(key_handler, TEXTVIEW_CALLBACK_KEYCODE_DOWN)) {
                    return true;
                }
            }
            else if (keyEvent->key() == Qt::Key_Left) {
                if (front_execute_key_callback(key_handler, TEXTVIEW_CALLBACK_KEYCODE_LEFT)) {
                    return true;
                }
            }
            else if (keyEvent->key() == Qt::Key_Right) {
                if (front_execute_key_callback(key_handler, TEXTVIEW_CALLBACK_KEYCODE_RIGHT)) {
                    return true;
                }
            }
        }

        return QTextEdit::eventFilter(obj, event);
    }

    void focusInEvent(QFocusEvent *e) override {
        QTextEdit::focusInEvent(e);
        front_set_token_binding(selected, 1, page_id);
    }

    void focusOutEvent(QFocusEvent *e) override {
        QTextEdit::focusOutEvent(e);
        front_set_token_binding(selected, 0, 0);
    }

    ~TextView() {
        front_free_token_binding(selected);
        front_free_textview_state(text_view_state);
        front_free_key_callback(key_handler);
    }

    fat_pointer text_view_state{};
    fat_pointer selected{};
    fat_pointer key_handler{};
    bool executing_back;
    int32_t page_id;
};

extern "C" void *
back_text_view_init()
{
    TextView *tv = new TextView();
    return tv;
}

extern "C" void
back_text_view_full_replace(
    void *tv,
    const uint8_t* with,
    fat_pointer text_view_state,
    fat_pointer selected,
    fat_pointer key_callback
)
{
    auto* textView = static_cast<TextView*>(tv);
    textView->executing_back = true;

    textView->setPlainText(QString::fromUtf8(reinterpret_cast<const char*>(with)));

    textView->text_view_state = text_view_state;
    textView->selected = selected;
    textView->key_handler = key_callback;

    textView->executing_back = false;
}

extern "C" void
back_text_view_replace(void *tv, size_t start, size_t len, const uint8_t* with)
{
    auto* textView = static_cast<TextView*>(tv);
    textView->executing_back = true;

    QTextCursor cursor(textView->document());
    cursor.setPosition(start);
    cursor.setPosition(start + len, QTextCursor::KeepAnchor);
    cursor.insertText(QString::fromUtf8(reinterpret_cast<const char*>(with)));

    textView->executing_back = false;
}

extern "C" void
back_text_view_set_font(void *tv, uint8_t const* font_path, double font_size)
{
    auto* textView = static_cast<TextView*>(tv);
    textView->executing_back = true;

    QFont font = getFont(font_path, font_size, false, false);
    textView->setFont(font);

    textView->executing_back = false;
}

extern "C" void
back_text_view_set_editing_state(void *tv, uint8_t editing)
{
    (void) tv;
    (void) editing;
}

extern "C" void
back_text_view_set_line_attributes(
    void *tv,
    size_t line_no, size_t start, size_t end,
    int justification_sign,
    double leading_indentation, double trailing_indentation
)
{
    (void) line_no;

    auto* textView = static_cast<TextView*>(tv);
    textView->executing_back = true;

    QTextCursor cursor(textView->document());
    cursor.setPosition(start);
    cursor.setPosition(end, QTextCursor::KeepAnchor);

    QTextBlockFormat blockFormat;

    if (justification_sign < 0) {
        blockFormat.setAlignment(Qt::AlignLeft);
    } else if (justification_sign == 0) {
        blockFormat.setAlignment(Qt::AlignCenter);
    } else {
        blockFormat.setAlignment(Qt::AlignRight);
    }

    blockFormat.setTextIndent(leading_indentation);
    blockFormat.setLeftMargin(leading_indentation);
    blockFormat.setRightMargin(trailing_indentation);

    cursor.mergeBlockFormat(blockFormat);

    textView->executing_back = false;
}

extern "C" void
back_text_view_set_char_attributes(
    void *tv, size_t start, size_t end,
    uint8_t bold, uint8_t italic, uint8_t underline, uint8_t strikethrough,
    color back, color front
)
{
    auto* textView = static_cast<TextView*>(tv);
    textView->executing_back = true;

    QTextCursor cursor(textView->document());
    cursor.setPosition(start);
    cursor.setPosition(end, QTextCursor::KeepAnchor);

    QTextCharFormat format;

    QFont font = cursor.charFormat().font();
    font.setBold(bold);
    font.setItalic(italic);
    format.setFont(font);

    format.setUnderlineStyle(underline ? QTextCharFormat::SingleUnderline : QTextCharFormat::NoUnderline);
    format.setFontStrikeOut(strikethrough);

    format.setBackground(QColor(back.r, back.g, back.b, back.a));
    format.setForeground(QColor(front.r, front.g, front.b, front.a));

    cursor.mergeCharFormat(format);

    textView->executing_back = false;
}

extern "C" void
back_text_view_set_selection(void *tv, size_t start, size_t len)
{
    auto* textView = static_cast<TextView*>(tv);
    textView->executing_back = true;

    QTextCursor cursor = textView->textCursor();
    cursor.setPosition(start);
    cursor.setPosition(start + len, QTextCursor::KeepAnchor);
    textView->setTextCursor(cursor);

    textView->executing_back = false;
}

extern "C" void
back_text_view_get_selection(void *tv, size_t *start, size_t* end)
{
    auto* textView = static_cast<TextView*>(tv);
    QTextCursor cursor = textView->textCursor();

    *start = cursor.selectionStart();
    *end = cursor.selectionEnd();
}

extern "C" double
back_text_view_get_line_height(void *tv, size_t line, size_t start, size_t end, double width)
{
    (void) line; (void) end; (void) width;

    auto* textView = static_cast<TextView*>(tv);
    QTextBlock block = textView->document()->findBlock(start);
    double ret = block.layout()->boundingRect().height();
    std::cerr << "Return Line Height " << ret << '\n';
    return ret;
}

extern "C" void
back_text_view_get_cursor_pos(void *tv, double *x, double *y)
{
    auto* textView = static_cast<TextView*>(tv);
    QTextCursor cursor = textView->textCursor();
    QRect rect = textView->cursorRect(cursor);

    *x = rect.x();
    *y = rect.y();
}

extern "C" void
back_text_view_set_page_id(void *tv, int32_t page_id)
{
    auto* textView = static_cast<TextView*>(tv);
    textView->page_id = page_id;
}

extern "C" void
back_text_view_focus(void *tv)
{
    auto* textView = static_cast<TextView*>(tv);
    textView->setFocus();
}

extern "C" void
back_text_view_unfocus(void *tv)
{
    auto* textView = static_cast<TextView*>(tv);
    textView->clearFocus();
}

extern "C" void
back_text_view_copy(void *tv)
{
    auto* textView = static_cast<TextView*>(tv);
    textView->copy();
}

extern "C" void
back_text_view_cut(void *tv)
{
    auto* textView = static_cast<TextView*>(tv);
    textView->cut();
}

extern "C" void
back_text_view_paste(void *tv)
{
    auto* textView = static_cast<TextView*>(tv);
    textView->paste();
}

extern "C" void
back_text_view_select_all(void *tv)
{
    auto* textView = static_cast<TextView*>(tv);
    textView->selectAll();
}