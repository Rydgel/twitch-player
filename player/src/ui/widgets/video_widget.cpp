#include "ui/widgets/video_widget.hpp"

#include "ui/overlays/video_controls.hpp"
#include "ui/overlays/video_details.hpp"
#include "ui/utils/event_notifier.hpp"
#include "ui/native/capabilities.hpp"

#include "libvlc/event_watcher.hpp"

#include "prelude/variant.hpp"
#include "prelude/timer.hpp"

#include "constants.hpp"

#include <QApplication>
#include <QWheelEvent>
#include <QMouseEvent>
#include <QShortcut>
#include <QSettings>

static auto quality_names(const StreamIndex & index) {
    QStringList qualities;

    for (auto pl_info: index.playlist_infos)
        qualities << pl_info.media_info.name;

    return qualities;
}

static const auto OVERLAY_INVALIDATING_EVENTS = {
    QEvent::Move,
    QEvent::KeyRelease,
};

VideoWidget::VideoWidget(libvlc::Instance &instance, QWidget *parent):
    QWidget(parent),
    _instance(instance),
    _media_player(libvlc::MediaPlayer(instance)),
    _details(new VideoDetails(this)),
    _controls(new VideoControls(this)),
    _event_watcher(new VLCEventWatcher(_media_player, this)),
    _retry_timer(new QTimer(this))
{
    auto notifier = new EventNotifier(OVERLAY_INVALIDATING_EVENTS, this);
    window()->installEventFilter(notifier);

    QObject::connect(notifier, &EventNotifier::new_event, [=](auto) {
        update_overlay_position();
    });

    setAttribute(Qt::WA_OpaquePaintEvent);
    setFocusPolicy(Qt::WheelFocus);
    _media_player.set_renderer((void *)winId());
    _media_player.set_volume(_vol);
    update_overlay_position();
    setMinimumSize(160, 90);
    setMouseTracking(true);

    _controls->set_volume(_vol);
    QObject::connect(_controls, &VideoControls::volume_changed, [=](int vol) {
        set_volume(vol);
        activateWindow();
    });
    QObject::connect(_controls, &VideoControls::muted_changed, [=](bool muted) {
        set_muted(muted);
    });
    QObject::connect(_controls, &VideoControls::fast_forward, [=] {
        fast_forward();
    });
    QObject::connect(_controls, &VideoControls::quality_changed, [=](auto quality) {
        play(_current_channel, quality);
        activateWindow();
    });

    _retry_timer->setSingleShot(true);
    _retry_timer->setInterval(1000);
    QObject::connect(_retry_timer, &QTimer::timeout, [=] {
        play(_current_channel, _current_quality);
        _retry_timer->setInterval(_retry_timer->interval() * 2);
    });

    QObject::connect(_event_watcher, &VLCEventWatcher::new_event, [=](auto event) {
        using namespace libvlc::events;

        auto set_buffering = [=](bool on) { _details->set_buffering(on); };
        auto schedule_refresh = [=] { _retry_timer->start(); };

        match(event,
            [=](Opening)          { set_buffering(true); },
            [=](Buffering b)      { set_buffering(b.cache_percent != 100.f); },
            [=](EndReached)       { schedule_refresh(); },
            [=](Stopped)          { schedule_refresh(); },
            [=](EncounteredError) { schedule_refresh(); },
            [=](auto)             { }
        );
    });
}

void VideoWidget::play(QString channel, QString quality) {
    _current_channel = channel;
    _current_quality = quality;

    auto location = TwitchdAPI::playback_url(channel, quality);

    _media.emplace(_instance, location.toStdString().c_str());
    _media_player.set_media(*_media);
    _media_player.play();

    _details->set_channel(channel);
    _controls->clear_qualities();

    _api.stream_index(channel).then([=](StreamIndex index) {
        auto qualities = quality_names(index);
        _controls->clear_qualities();
        _controls->set_qualities(quality, qualities);
        _retry_timer->setInterval(1000);
    });

    _details->show();

    if (!quality.isEmpty()) {
        using namespace constants::settings::streams;
        QSettings settings;
        settings.setValue(KEY_LAST_QUALITY_FOR(channel), quality);
    }
}

int VideoWidget::volume() const {
    return _vol;
}

void VideoWidget::set_volume(int volume) {
    _vol = volume;
    _media_player.set_volume(_muted ? 0 : _vol);
    _controls->set_volume(_vol);
    _details->show_state(QString("%1 %").arg(_vol));
}

bool VideoWidget::muted() const {
    return _muted;
}

void VideoWidget::set_muted(bool muted) {
    _muted = muted;
    set_volume(_vol);
    _controls->set_muted(_muted);
    _details->show_state(muted ? "Muted" : "Unmuted");
}

void VideoWidget::fast_forward() {
    _media_player.set_position(std::nextafter(1.0f, 0.f));
    _details->show_state("Fast forward...");
}

libvlc::MediaPlayer & VideoWidget::media_player() {
    return _media_player;
}

void VideoWidget::update_overlay_position() {
    auto top_left = mapToGlobal(pos()) - pos();
    auto bottom_left = top_left + QPoint(0, height());

    _details->resize(size());
    _details->move(top_left);
    _controls->resize(width(), _controls->height());
    _controls->move(bottom_left - QPoint(0, _controls->height()));
}

void VideoWidget::wheelEvent(QWheelEvent *event) {
    setFocus();
    auto delta = qApp->keyboardModifiers().testFlag(Qt::ShiftModifier)
               ? 1 : 5;
    int new_volume;
    if (event->angleDelta().y() > 0)
        new_volume = std::min(_vol + delta, 200);
    else
        new_volume = std::max(0, _vol - delta);
    set_volume(new_volume);
    QWidget::wheelEvent(event);
}

void VideoWidget::resizeEvent(QResizeEvent *event) {
    update_overlay_position();
    QWidget::resizeEvent(event);
}

void VideoWidget::showEvent(QShowEvent *event) {
    // have to actually wait for it to be shown ...
    delayed(this, 250, [this] { update_overlay_position(); });
    QWidget::showEvent(event);
}

void VideoWidget::mousePressEvent(QMouseEvent *event) {
    _controls->appear();
    _details->show_stream_details();

    if (event->buttons().testFlag(Qt::MouseButton::LeftButton))
        _last_drag_position = event->globalPos();

    QWidget::mousePressEvent(event);
}

void VideoWidget::mouseMoveEvent(QMouseEvent *event) {
    _controls->appear();
    _details->show_stream_details();

    if (event->buttons().testFlag(Qt::MouseButton::LeftButton)) {
        auto delta = event->globalPos() - _last_drag_position;
        if (delta.manhattanLength() > 10) {
            window()->move(window()->pos() + delta);
            _last_drag_position = event->globalPos();
        }
    }
    else
        QWidget::mouseMoveEvent(event);
}
