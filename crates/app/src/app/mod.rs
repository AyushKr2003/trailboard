//! Board state and interaction: cards are live pages on an infinite canvas.
//!
//! Input model
//! - Drag empty board / middle-drag anywhere: pan. Wheel over board: pan. Ctrl+wheel: zoom.
//! - Click a card's page: focus it and give it the keys (and the wheel). Drag its title
//!   bar to move, its bottom-right corner to resize; × closes; double-click the title bar
//!   to fill the view. A title-bar click makes the card current without the keys.
//! - Keyboard first (Omarchy's app convention): keys are board keys unless a page has
//!   them (`App::typing`). h/j/k/l or arrows go to the nearest card that way
//!   (`scene::nav`), Shift with them pans, +/- zoom, Enter/Space give the current card
//!   the keys, Esc takes them back (the card stays current) and a second Esc lets go
//!   of the card, Delete or d closes it (u reopens), Q quits. Board keys (f, N, T, S, O, R, C, [, ])
//!   act on the current card, else the one in the middle of the view.
//! - Ctrl+B opens the trails sidebar (`scene::sidebar`, `sidebar`) with the keys:
//!   j/k move its highlight and go to that card, h/l collapse / expand a trail,
//!   Enter gives a card's page the keys (or toggles a trail), r names the
//!   row's trail, Esc or B closes it. Clicks on a row do the same (a
//!   double-click on a trail's header names it); a click elsewhere takes the keys back, the
//!   panel stays open. The pointer resting on the window's left edge peeks it
//!   without the keys (`sidebar::Peek`); it goes once the pointer leaves, and
//!   B keeps it. The chips at its foot switch spaces.
//! - f labels the links and controls on the current card's page (`scene::hint`,
//!   `web::links`); typing a label opens a link beside the card, which becomes
//!   current, or clicks a control (a field also gets the keys). Esc, another
//!   key, a click or a scroll ends the labels. Shift+F fits everything.
//! - Mouse-only actions have keys too: v selects / deselects the current card
//!   (h/j/k/l keep the selection), z fills the view with it, Ctrl+d/u scroll its
//!   page, and the Menu key or Shift+F10 opens its page menu (Chromium picks
//!   the spot and context; j/k, Enter, Esc in menus).
//! - M arranges the current card (`scene::arrange`): h/j/k/l move it (the
//!   selection with it), Shift resizes, Ctrl takes fine steps; Enter/M keep the
//!   result as one undo step, Esc puts it back, any other key or a click keeps it.
//! - Ctrl+K palette (`palette`, `scene::palette`): fuzzy search over every card
//!   (folded ones too), the other spaces and the board commands; Up/Down pick,
//!   Enter goes there or runs it, the last row searches the web or opens an address.
//! - Ctrl+H history view (`palette::history_rows`): visited pages newest first,
//!   filtered by words; Shift+Delete forgets one (in Ctrl+K too); *Clear history*
//!   asks first (`Confirm`).
//! - Ctrl+, settings (`scene::settings`, saved by `storage::settings`): a
//!   settings page card (`settings_page`, `scene::note::SETTINGS_URL`) with
//!   sections, switches and drop-downs, found again if it is on the board. Each
//!   change applies at once
//!   (browsing ones through `web::prefs`, `web::permission` and the per-site
//!   JavaScript and images of `scene::sitesettings`).
//! - Ctrl+J downloads (`palette::download_rows`, `scene::download`): files go
//!   straight to the Downloads folder; Enter opens one, Ctrl+Enter its folder,
//!   Shift+Delete cancels or removes. Pages with running downloads are parked.
//! - Ctrl+L address bar, Ctrl+T new card, Ctrl+Shift+T reopen, Ctrl+R reload,
//!   Alt+←/→ back/forward. While typing in a page, `keys::typing_key` goes
//!   first: Ctrl+W / Ctrl+U / Ctrl+H edit the field and Ctrl+R is the page's.
//! - Ctrl+F find in the current card's page (the bar is an `omnibox::Target::Find`;
//!   `sync_find` stops the search when it closes). Ctrl +/−/0 zoom the current
//!   page per site (`scene::pagezoom`, saved by `storage::zoom`, applied by
//!   `web::set_page_zoom`); with no page current Ctrl+0 fits all.
//! - Ctrl+Z / Ctrl+Shift+Z (or Ctrl+Y) undo/redo board edits (open, close, move,
//!   resize). After clicking inside a page they go to the page instead (text undo).
//! - Shift+drag on the board draws a selection band; Shift+click a title bar adds or
//!   removes that card; Ctrl+A (no card focused) selects all. Dragging a selected
//!   card's title bar moves the whole selection; Delete/Backspace closes it. A click
//!   on the board or Esc clears it.
//!
//! - F1, `:keys`, or `?` on the board shows a sheet of shortcuts; Esc, F1, `?` or a click hides it.
//! - Right-click in a page opens a context menu (drawn by the app); Esc, a click
//!   outside, scrolling or moving the board closes it.
//! - Ctrl+C / Ctrl+X / Ctrl+V in a page go through the desktop clipboard (see `clipboard`).
//! - IME (e.g. fcitx5) composition goes to the focused page or the address bar.
//! - `?` in a page finds backward (`/` forward); Esc clears the marks a find left.
//! - `:messages` lists this session's toasts and the `:later` lines waiting; Ctrl+Q quits.
//! - Ctrl+S saves the focused note, or else the whole board, as Markdown in
//!   `Documents/Trailboard` (`scene::export`, `storage::export`); a note's page
//!   menu has *Save as Markdown* too.
//! - Tab (unless keys are going to a page) toggles Solo: the focused card, or
//!   the one in the middle of the view, fills the window and its page lays out at
//!   the window's size. Esc or Tab returns to the board.
//! - Looks follow Omarchy live (see `theme` and `desktop`): the theme's palette,
//!   light or dark (pages too), Hyprland's rounding and the desktop text size.
//! - Spaces: Ctrl+Shift+N new space, Ctrl+PageDown / Ctrl+PageUp next / previous,
//!   F2 rename, Ctrl+Shift+B next tint (a theme colour), Ctrl+Shift+W delete (asks
//!   first when the space has cards), Ctrl+Shift+P private on / off. Each space
//!   keeps its own board, camera, background, undo history and closed cards. A
//!   private space's pages are not recorded (history, icons, zoom on disk) and
//!   share a cookie jar in memory (`web::Jar`) that lasts until the app quits.
//!
//! Live-browser cap: at most *Pages kept awake* (a setting, default 6;
//! `TRAILBOARD_MAX_LIVE` overrides it for a run) cards keep a live
//! browser (see `scene::liveness`). The others sleep: their page is closed and the
//! card shows its last frame, dimmed, until it is focused or scrolled into view.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::rc::Rc;
use std::time::{Duration, Instant};

use compositor::{Layer, Renderer};
use scene::arrange::Arrange;
use scene::jump::{self, Before, ScrollSpot};
use scene::pagezoom::SiteZooms;
use scene::settings::Settings;
use scene::{Camera, caret, dock, Edge, hint, History, keys, motion, Node, NodeId, read, Rect, Scene, scrolltarget, Selection, userkeys, Vec2};
use storage::spaces::{SpaceId, SpaceIndex};
use web::{MouseButton as WebButton, Page, SpawnBeside};
use winit::event::MouseButton;
use winit::keyboard::ModifiersState;
use winit::window::CursorIcon;

use crate::board::camera::Gather;
use crate::board::spaces::Session;
use crate::dev::script::Script;
use crate::keyboard::jumps::{SectionAsk, SpotAsk};
use crate::keyboard::page_nav::CaretCard;
use crate::keyboard::scroll_sections::ScrollFlash;
use crate::page::clipboard::Clipboard;
use crate::page::dialogs::PageDialog;
use crate::page::downloads::SaveAs;
use crate::page::editor::Editing;
use crate::page::file_chooser::Choosing;
use crate::page::inspector::{Dock, DockParts};
use crate::system::desktop::Desktop;
use crate::trails::joins::CardPick;
use crate::ui::menu::{self, ContextMenu, Entry};
use crate::ui::messages::LaterLine;
use crate::ui::omnibox::Omnibox;
use crate::ui::palette;
use crate::ui::theme::{rgb, Theme};

mod drawing;
mod geometry;
mod input;
mod look;
mod page_events;
mod persist;
mod setup;
mod typing;
mod window_events;

pub(crate) const MIN_CARD: f32 = 240.0;
/// How long a half-typed sequence waits before the panel of what can
/// follow it shows (typed quickly, `gg` never shows it).
pub(crate) const PREFIX_PANEL_AFTER: Duration = Duration::from_millis(400);
/// The longest the event loop sleeps with nothing scheduled: [`App::tick`]'s
/// polled timers run this often.
pub(crate) const IDLE_TICK: Duration = Duration::from_millis(250);
/// The longest between draws with nothing scheduled (see
/// `App::next_deadline`): what the draw itself polls (the wallpaper file,
/// a board photo decoding) shows within this. Not every tick: a frame built
/// to be skipped as unchanged 4 times a second was most of idle's CPU.
pub(crate) const IDLE_DRAW: Duration = Duration::from_secs(1);
/// How long after a page loads (or retitles itself) its text is read for history.
pub(crate) const TEXT_SETTLE: Duration = Duration::from_millis(1200);
/// How long the outline around a newly chosen scroll section shows.
pub(crate) const SCROLL_FLASH: Duration = Duration::from_millis(900);
/// One keyboard move or resize step (M, then h/j/k/l), in logical px on screen.
pub(crate) const ARRANGE_STEP: f32 = 40.0;
/// The fine step with Ctrl.
pub(crate) const ARRANGE_FINE: f32 = 8.0;
pub(crate) const ARRANGE_HELP: &str = "Moving · h j k l move · Shift resize · Ctrl fine · Enter done · Esc cancel";
pub(crate) const RESIZE_GRIP: f32 = 18.0;
/// A card's name sits on a label above it, the same size on screen at any
/// zoom: its site's icon and title, and under the pointer (or on the current
/// card) its buttons. The label is the card's handle: drag it to move the
/// card. Sizes in px, times the UI scale.
pub(crate) const BAR_H: f32 = 26.0;
/// Between the label and the card.
pub(crate) const BAR_GAP: f32 = 6.0;
/// Width of a label button (close, reload, forward, back).
pub(crate) const BAR_BTN: f32 = 24.0;
/// A card narrower than this on screen has no room for back, forward and
/// reload on its label; narrower than `BAR_CLOSE_MIN`, none for close either.
pub(crate) const BAR_NAV_MIN: f32 = 280.0;
pub(crate) const BAR_CLOSE_MIN: f32 = 96.0;
/// Below this zoom every card shows its label (landmarks on a far-out
/// board); above it only the current card, the one under the pointer and
/// selected ones do.
pub(crate) const LABELS_ALL_BELOW: f32 = 0.45;
/// Height of the Solo title bar, in overlay px (scaled by `ui()`).
/// None: the page fills the whole window (Tab or Esc goes back).
pub(crate) const READ_BAR: f32 = 0.0;

/// A card moving between the board and Solo (`scene::read::morph`).
#[derive(Debug, Clone, Copy)]
pub(crate) struct ReadMorph {
    pub(crate) id: NodeId,
    /// Growing into Solo (else shrinking back to its place on the board).
    pub(crate) entering: bool,
    pub(crate) start: Instant,
}

impl ReadMorph {
    /// How far into Solo the card is: 0 on the board, 1 filling the window.
    pub(crate) fn progress(&self) -> f32 {
        let t = crate::ui::chrome::ease_out(self.start.elapsed().as_secs_f32() * 1000.0 / read::MORPH_MS as f32);
        if self.entering { t } else { 1.0 - t }
    }

    pub(crate) fn done(&self) -> bool {
        self.start.elapsed() >= Duration::from_millis(read::MORPH_MS)
    }
}
/// How long a toast stays on screen.
pub(crate) const TOAST_FOR: Duration = Duration::from_millis(1800);
/// Bits of a page key that hold the card id; the space epoch sits above them.
pub(crate) const KEY_ID_BITS: u32 = 40;

/// Texture keys of site icons: this bit set, so they never meet a page key.
pub(crate) const ICON_KEY: u64 = 1 << 63;
/// Texture key of the page preview beside the palette's list.
pub(crate) const PREVIEW_KEY: u64 = 1 << 62;
/// Sleeping cards given their page's thumbnail per frame, at most (each is a
/// database read and a JPEG decode).
pub(crate) const THUMBS_PER_FRAME: usize = 4;

/// The page preview beside the palette's list: whose it is, and its size
/// once uploaded as [`PREVIEW_KEY`] (`None` when the page has no thumbnail).
pub(crate) struct Preview {
    pub(crate) url: String,
    pub(crate) size: Option<(u32, u32)>,
}

/// A site's icon, uploaded as texture `key` whenever `serial` changes.
pub(crate) struct SiteIcon {
    pub(crate) key: u64,
    pub(crate) serial: u64,
    pub(crate) icon: storage::icons::Icon,
    /// Written under `icons/` (not when only a private space has seen it).
    pub(crate) saved: bool,
}

/// Browser and texture key for card `id` while `epoch` is current.
pub(crate) fn page_key(epoch: u64, id: NodeId) -> u64 {
    (epoch << KEY_ID_BITS) | id.0
}

/// Set in a page key for the inspector docked in that card (`App::docks`).
pub(crate) const DOCK_BIT: u64 = 1 << (KEY_ID_BITS - 1);

/// Browser and texture key for the inspector docked in card `id`.
pub(crate) fn dock_key(epoch: u64, id: NodeId) -> u64 {
    page_key(epoch, id) | DOCK_BIT
}

/// A wheel notch in the docked inspector, px.
pub(crate) const WHEEL_LINE: f32 = 100.0;

/// Width of the divider between a page and its docked inspector, CSS px.
pub(crate) const DOCK_GAP: f32 = 6.0;

#[derive(Debug, Clone, Copy)]
pub(crate) enum Drag {
    None,
    Pan { last: Vec2 },
    /// Title-bar drag of `id`; `App::group` holds the start rects of every card it moves.
    Move { id: NodeId, grab: Vec2 },
    /// Selection band from `anchor` (world) to the cursor.
    Band { anchor: Vec2 },
    Resize { id: NodeId, anchor: Vec2, start: Rect },
    /// Mouse is down inside a page; moves and release go to it.
    Page { id: NodeId, button: WebButton },
    /// The same inside card `id`'s docked inspector.
    Dock { id: NodeId, button: WebButton },
    /// Dragging the divider between card `id`'s page and its inspector.
    Divider { id: NodeId },
    /// Alt+drag of card `id`'s label: `App::group` holds the cards that
    /// will join the trail of the card it is let go on (M15); `keep`
    /// (Alt+Shift) keeps their colour.
    Join { id: NodeId, keep: bool, link: bool },
}

/// Link hints on card `id`: its visible links (rects in the page view's CSS
/// px) and their labels, `None` until the page has answered.
/// A press on a trail's header row in the sidebar (`App::sidebar_press`).
#[derive(Debug, Clone, Copy)]
pub(crate) struct SidebarPress {
    /// The trail, by its first card.
    pub(crate) head: NodeId,
    pub(crate) at: Vec2,
    /// Moved far enough to be a drag rather than a click.
    pub(crate) dragging: bool,
    /// Where the trail would go among its part's trails if let go now.
    pub(crate) to: Option<usize>,
}

/// Where an entered card's keys go (M12). Board mode is no card entered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PageKeys {
    /// Our page keys first (`scene::keys::page`); keys that type nothing go on.
    Page,
    /// Every key to the page; Esc back to Page.
    Insert,
    /// Every key to the page, Esc included; Shift+Esc back to Page.
    Passthrough,
    /// A caret in the page's text, selecting or not (`v`, `V`; step 14).
    Visual(caret::Mode),
}

impl PageKeys {
    pub(crate) fn label(self) -> &'static str {
        match self {
            PageKeys::Page => "Page",
            PageKeys::Insert => "Insert",
            PageKeys::Passthrough => "Passthrough · Shift+Esc to leave",
            PageKeys::Visual(m) => m.label(),
        }
    }
}

pub(crate) struct LinkHints {
    pub(crate) id: NodeId,
    /// Follow a link in the same card (Page mode's f) instead of beside it.
    pub(crate) here: bool,
    /// `;` hints: what choosing a label does instead of following it.
    pub(crate) kind: Option<hint::Kind>,
    /// Labels already chosen while `kind` keeps the labels up (`;m`).
    pub(crate) chosen: Vec<usize>,
    /// `:hint`: the command line a chosen link runs (`{hint-url}`).
    pub(crate) line: Option<String>,
    pub(crate) links: Vec<(Rect, web::links::Action)>,
    /// `s`: labels on the page's scroll sections instead of its links
    /// (section number, where it is in the view's CSS px), in label order.
    pub(crate) sections: Vec<(u32, Rect)>,
    pub(crate) hints: Option<hint::Hints>,
}

pub(crate) struct Card {
    pub(crate) page: Page,
    pub(crate) loading: bool,
    /// The page has history to go back / forward to (its title bar's arrows).
    pub(crate) can_back: bool,
    pub(crate) can_forward: bool,
    pub(crate) hidden: bool,
    /// Cursor the page asked for, shown while the pointer is over its content.
    pub(crate) cursor: web::Cursor,
    /// Frames a second last asked of the page (`sync_pages`).
    pub(crate) fps: i32,
    /// The page holds its focus (an *Always active* card).
    pub(crate) held: bool,
}

/// What a card's page last said about its media (`WebEvent::Media`).
#[derive(Debug, Clone, Copy)]
pub(crate) struct Media {
    pub(crate) playing: bool,
    /// When it last stopped playing; `None` while playing.
    pub(crate) stopped: Option<Instant>,
}

/// Frames a second for a page that keeps running off screen without being
/// seen (media in an *Auto* or *Keep awake* card): enough to keep it going,
/// little to pay for.
pub(crate) const UNSEEN_FPS: i32 = 4;
/// Frames a second for a page in view, or an *Always active* one.
pub(crate) const FULL_FPS: i32 = 60;

/// The gap between a stack's sheets on screen, px: it follows the zoom but
/// stays a few px, so a stack still reads when zoomed far out.
pub(crate) fn stack_step_px(k: f32, scale: f32) -> f32 {
    (12.0 * k).max(5.0 * scale)
}

/// Appear animations of the overlays.
#[derive(Default)]
pub(crate) struct Anims {
    pub(crate) tip: crate::ui::chrome::Appear,
    pub(crate) omnibox: crate::ui::chrome::Appear,
    pub(crate) menu: crate::ui::chrome::Appear,
    pub(crate) toast: crate::ui::chrome::Appear,
    pub(crate) question: crate::ui::chrome::Appear,
    pub(crate) sheet: crate::ui::chrome::Appear,
    pub(crate) sidebar: crate::ui::chrome::Appear,
    pub(crate) welcome: crate::ui::chrome::Appear,
}

/// A small hash for telling one overlay's content from the next.
pub(crate) fn anim_key<T: std::hash::Hash>(v: T) -> u64 {
    use std::hash::{BuildHasher, BuildHasherDefault, DefaultHasher};
    BuildHasherDefault::<DefaultHasher>::default().hash_one(v)
}

pub(crate) struct App {
    pub(crate) renderer: Option<Renderer>,
    pub(crate) scene: Scene,
    pub(crate) camera: Camera,
    /// Camera we're gliding toward (fit / Solo).
    pub(crate) camera_goal: Option<Camera>,
    /// The cards the opening camera was fitted to, and that camera: until
    /// the camera moves or input comes, a resize fits them again (the window
    /// manager's size arrives after the first fit, at the requested one).
    pub(crate) opening_fit: Option<(Rect, Camera)>,
    /// Wheel and touchpad scrolling inside a page, and flings (`scene::scroll`):
    /// to card, at view px.
    pub(crate) page_scroll: scene::scroll::Scroller<(NodeId, i32, i32)>,
    /// Wheel and touchpad panning of the board: glides and flings (`scene::scroll::Pan`).
    pub(crate) board_pan: scene::scroll::Pan,
    /// Live browsers. A card in the scene but not here is asleep.
    pub(crate) cards: HashMap<NodeId, Card>,
    /// Inspector cards and the card each inspects. An inspector lives only
    /// while both run (`check_inspectors`); it never sleeps and is never saved.
    pub(crate) inspectors: HashMap<NodeId, NodeId>,
    /// Inspectors docked in their page's card, by card. Like inspector
    /// cards they live only while their page does and are never saved.
    pub(crate) docks: HashMap<NodeId, Dock>,
    /// The keys go to the focused card's docked inspector, not its page.
    pub(crate) dock_keys: bool,
    /// Where the next docked inspector opens (the side used last).
    pub(crate) dock_side: dock::Side,
    pub(crate) max_live: usize,
    /// The user's settings (Ctrl+,), saved in `settings.json`.
    pub(crate) settings: Settings,
    /// Ad and tracker blocking: filter lists and the sites allowed.
    pub(crate) blocking: crate::system::blocking::Blocking,
    /// `TRAILBOARD_MAX_LIVE` set the live-page cap for this run (until the setting changes).
    pub(crate) live_from_env: bool,
    /// Recency of use per card (larger is more recent), for choosing which cards sleep.
    pub(crate) used: HashMap<NodeId, u64>,
    /// What each live card's page says about its media, for keeping it awake.
    pub(crate) media: HashMap<NodeId, Media>,
    /// The card a media control was last sent to, and when: only its
    /// answer, soon after, becomes a toast.
    pub(crate) media_asked: Option<(NodeId, Instant)>,
    /// "Can't play" messages shown: card, kind, page address (once each).
    pub(crate) media_told: HashSet<(NodeId, scene::media::Trouble, String)>,
    /// The card whose page is in full screen, shown in Solo, and the card
    /// Solo had before (`None`: the board), to go back to after.
    pub(crate) page_full: Option<(NodeId, Option<NodeId>)>,
    pub(crate) use_clock: u64,
    pub(crate) focused: Option<NodeId>,
    /// The card current before `focused`, for Ctrl+^ (this space's).
    pub(crate) alt_card: Option<NodeId>,
    /// Closed cards for Ctrl+Shift+T, newest last.
    pub(crate) closed: Vec<(Node, Vec<Edge>)>,
    pub(crate) history: History,
    pub(crate) selection: Selection,
    /// Cards moved by the current title-bar drag, with their start rects.
    pub(crate) group: Vec<(NodeId, Rect)>,
    /// The last click landed inside the focused page, so its keys (incl. Ctrl+Z) go there.
    pub(crate) keys_in_page: bool,
    /// Every mode's key bindings (`scene::keys`, M12), with the key
    /// file's and `:bind`'s (`scene::userkeys`, step 10).
    pub(crate) keys: userkeys::Keys,
    /// The `?` sheet's sections, built from `keys` when it opens.
    pub(crate) sheet_sections: Vec<crate::keyboard::key_sheet::Section>,
    /// A count or sequence typed toward a binding (`5j`).
    pub(crate) pending: keys::Pending,
    /// Where an entered card's keys go.
    pub(crate) page_keys: PageKeys,
    /// Ctrl+G during a half-typed sequence: the time it began, so the list
    /// of what can follow shows until it ends (`mode-label` Ring).
    pub(crate) prefix_asked: Option<Instant>,
    /// What the window title says now (`App::update_title`).
    pub(crate) title_shown: String,
    /// The card Caret / Visual mode is in, and whether a copy was asked
    /// for (only then is a copy from the page taken).
    pub(crate) caret: Option<CaretCard>,
    /// Keys pressed in Page mode that went on to the page, so their release does too.
    pub(crate) forwarded: HashSet<i32>,
    pub(crate) omnibox: Option<Omnibox>,
    /// Lines run on the `:` command line, newest first (`storage::commands`).
    pub(crate) recent_commands: scene::command::Recent,
    /// The last line run on the command line this session (private spaces
    /// too), for `.`.
    pub(crate) last_line: Option<String>,
    /// `:later` lines waiting: when, the chain, the hint's link it had.
    pub(crate) later: Vec<LaterLine>,
    /// The last `LaterLine::id` given.
    pub(crate) later_ids: u64,
    /// Every toast this session, for `:messages`.
    pub(crate) messages: scene::messages::Messages,
    /// Every space's jump list and marks (`scene::jump`, `storage::jumps`).
    pub(crate) places: jump::Places,
    pub(crate) places_dirty: bool,
    /// Ctrl+o / Ctrl+i are moving: the card gone to isn't a new visit.
    pub(crate) jumping: bool,
    /// Where the last jump left from (`''`).
    pub(crate) before: Option<Before>,
    /// A scroll spot asked of a card's page, and what for.
    pub(crate) spot_ask: Option<(NodeId, SpotAsk)>,
    /// A mark's scroll spot to go to once the card's page has loaded.
    pub(crate) restore_due: HashMap<NodeId, ScrollSpot>,
    /// Saved pages (`scene::saved`, `storage::saved`).
    pub(crate) saved_pages: scene::saved::Saved,
    pub(crate) menu: Option<ContextMenu>,
    /// Where the title-bar shields were drawn last frame (screen px, with a
    /// little room around them), for clicks.
    pub(crate) shields: Vec<(NodeId, Rect)>,
    /// Where the trail tags were drawn last frame (screen px), in draw
    /// order, for clicks: a click on one names its trail.
    pub(crate) tags: Vec<(NodeId, Rect)>,
    /// The shortcut sheet (`?`) is showing.
    pub(crate) hints: bool,
    /// Its filter and scroll.
    pub(crate) sheet: crate::keyboard::key_sheet::Sheet,
    /// The sidebar's rows arriving and its tab gliding (`sidebar::Motion`).
    pub(crate) sidebar_motion: crate::trails::panel::Motion,
    /// Link hints (`f`) on a card's page: asked for, or showing labels.
    pub(crate) link_hints: Option<LinkHints>,
    /// Picking a card by its letters to join its trail (M15).
    pub(crate) card_pick: Option<CardPick>,
    /// Each card's chosen scroll section (`web::scroll`); a card not here
    /// scrolls the automatic pick. Forgotten when its page navigates.
    pub(crate) scroll_targets: HashMap<NodeId, u32>,
    /// The sections a card reported last, for naming them in the pill.
    pub(crate) sections: HashMap<NodeId, Vec<scrolltarget::Section>>,
    /// A section list asked of a card, and what for.
    pub(crate) section_ask: Option<(NodeId, SectionAsk)>,
    /// `]]` / `[[` asked of a card's page: the card, and next (true) or previous.
    pub(crate) pager_ask: Option<(NodeId, bool)>,
    /// What the last scroll key moved, for the mode pill (`section 2.1 · 40%`):
    /// the card, the section, and what to say after its name.
    pub(crate) scroll_note: Option<(NodeId, u32, String)>,
    pub(crate) scroll_flash: Option<ScrollFlash>,
    /// Moving or resizing the current card from the keyboard (M).
    pub(crate) arrange: Option<Arrange>,
    /// Solo: this card fills the window (see `scene::read`).
    pub(crate) read: Option<NodeId>,
    /// The card growing into Solo or shrinking back to the board.
    pub(crate) read_morph: Option<ReadMorph>,
    /// The CSS size each live card's last uploaded frame was laid out at, so
    /// a frame from before a resize is drawn at its own size, not stretched.
    pub(crate) page_css: HashMap<NodeId, (f32, f32)>,
    /// The same for each docked inspector.
    pub(crate) dock_css: HashMap<NodeId, (f32, f32)>,
    /// The trails sidebar (Ctrl+B): collapsed groups, highlight and scroll.
    pub(crate) sidebar: scene::sidebar::Sidebar,
    pub(crate) sidebar_open: bool,
    /// The sidebar has the keys (j/k move its highlight).
    pub(crate) sidebar_keys: bool,
    /// Index of the sidebar's highlight when last seen, for a row that goes away.
    pub(crate) sidebar_at: Option<usize>,
    /// Sidebar row under the pointer.
    pub(crate) sidebar_hover: Option<usize>,
    /// A press on a trail's header in the sidebar: a click toggles the group,
    /// a drag moves the trail.
    pub(crate) sidebar_press: Option<SidebarPress>,
    /// The last click that toggled a trail's header, for a double-click
    /// (which names the trail instead).
    pub(crate) sidebar_toggled: Option<(Instant, NodeId)>,
    /// The sidebar was opened by the pointer on the window's edge, and goes
    /// when the pointer leaves it (`sidebar::Peek`).
    pub(crate) sidebar_peek: bool,
    pub(crate) peek: crate::trails::panel::Peek,
    /// The space chip under the pointer in the sidebar's foot.
    pub(crate) sidebar_spot: Option<crate::trails::panel::Spot>,
    /// The current mouse press started inside the context menu.
    pub(crate) menu_pressed: bool,
    pub(crate) clipboard: Clipboard,
    /// IME input is enabled on the window.
    pub(crate) ime_allowed: bool,
    /// Page with an IME composition in progress.
    pub(crate) ime_page: Option<NodeId>,
    /// Last IME candidate-window area sent to the window (screen px).
    pub(crate) ime_area: Option<[i32; 4]>,
    pub(crate) spawn_beside: SpawnBeside,
    pub(crate) drag: Drag,
    pub(crate) cursor: Vec2,
    pub(crate) mods: ModifiersState,
    pub(crate) last_click: Option<(Instant, Vec2, MouseButton)>,
    pub(crate) click_count: i32,
    /// When the camera last changed (used to re-rasterize pages at the settled zoom).
    pub(crate) camera_changed: Instant,
    pub(crate) page_scale: f32,
    /// Pages asked for on the command line, opened once the window exists.
    pub(crate) initial_urls: Vec<String>,
    pub(crate) started: Instant,
    pub(crate) script: Option<Script>,
    pub quit: bool,
    /// The last draw presented a frame laid out differently from the one
    /// before: something may still be moving, so the loop draws again at the
    /// display's rate. (A frame where only page pixels changed doesn't: the
    /// page's next paint asks for the next one.)
    pub(crate) frame_changed: bool,
    /// Something changed outside a window event or a page's (a toast from a
    /// polled timer): draw on this pass ([`App::take_draw_wanted`]).
    pub(crate) draw_wanted: bool,
    /// A window event came since the loop last asked ([`App::take_event`]).
    pub(crate) event_seen: bool,
    /// Present every frame, as before frames were compared (`TRAILBOARD_REDRAW=always`).
    pub always_redraw: bool,
    /// `TRAILBOARD_PERF`: the cost of drawing, logged once a second.
    pub perf: Option<crate::dev::perf::PerfLog>,
    pub(crate) data_dir: std::path::PathBuf,
    pub(crate) spaces: SpaceIndex,
    /// False when the space index could not be read safely; nothing is saved then.
    pub(crate) persist: bool,
    /// Bumped on every space switch; part of each page's key, so late events and
    /// textures from the previous space's pages never land on this space's cards.
    pub(crate) epoch: u64,
    /// Undo history and closed cards of spaces not on screen (this session only).
    pub(crate) sessions: HashMap<SpaceId, Session>,
    /// The cookie jar of the active space when it is private.
    pub(crate) jar: Option<web::Jar>,
    /// Short message at the top of the window, until it expires.
    /// Message and when it goes.
    pub(crate) toast: Option<(String, Instant)>,
    /// How Chromium's sandbox confines pages, once `note_sandbox` is told.
    pub(crate) sandbox: Option<web::sandbox::Sandbox>,
    /// How far each overlay has come in (see `chrome::Appear`).
    pub(crate) anim: Anims,
    /// Cards' labels fading in and out (quiet labels).
    pub(crate) label_fades: crate::ui::chrome::Fades,
    /// The board's photo (*Board background: Photo*), read off the main thread.
    pub(crate) photo: crate::board::backdrop::Photo,
    /// The desktop's wallpaper file, looked up now and then (`backdrop::desktop_wallpaper`).
    pub(crate) wallpaper: Option<PathBuf>,
    pub(crate) wallpaper_checked: Option<Instant>,
    /// Pictures the photo list offers, found when it opens.
    pub(crate) photo_files: Vec<PathBuf>,
    /// Waiting for Enter to do something that can't be undone.
    pub(crate) confirm: Option<Confirm>,
    /// Board changed since the last save.
    pub(crate) dirty: bool,
    pub(crate) last_save: Instant,
    /// The Omarchy look in effect, and what follows the desktop for changes.
    pub(crate) theme: Theme,
    pub(crate) desktop: Option<Desktop>,
    /// Page colour scheme last sent to pages (dark).
    pub(crate) page_scheme: Option<bool>,
    /// The note "Add to note" adds to: the one last focused, edited or made.
    pub(crate) note_target: Option<NodeId>,
    /// Files saved as Markdown this session, per space and note (`None`: the
    /// whole board), so saving again overwrites them.
    pub(crate) saved: HashMap<(SpaceId, Option<NodeId>), std::path::PathBuf>,
    /// Page zoom per site (Ctrl +/−/0), shared with `web` (`web::set_page_zoom`).
    pub(crate) zooms: Rc<RefCell<SiteZooms>>,
    /// Site icons by `scene::icon::site`, for every space, saved under `icons/`.
    pub(crate) icons: HashMap<String, SiteIcon>,
    /// The start page's pins and parts, shared by every space (`start.json`).
    pub(crate) start_page: scene::startpage::StartPage,
    /// Whether the pointer is over the window (for the zoomed-out name label).
    pub(crate) pointer_in: bool,
    /// The page find in page last searched (to stop it when the bar closes).
    pub(crate) finding: Option<NodeId>,
    /// Find in page's last result: matches and the active one (1-based).
    pub(crate) found: Option<(i32, i32)>,
    /// What find in page last looked for; the bar opens with it.
    pub(crate) last_find: String,
    /// The last search went up the page (`?`): n goes up too, N down.
    pub(crate) find_back: bool,
    /// The card whose page still shows find marks (after n / N, or the
    /// bar closing on a match); Esc clears them before doing anything else.
    pub(crate) find_marks: Option<NodeId>,
    /// When the Menu key (or Shift+F10) asked a page for its menu: the menu
    /// that answers opens with its first item highlighted.
    pub(crate) menu_by_key: Option<Instant>,
    /// The history index (`page-text.db`); `None` if it could not be opened.
    pub(crate) page_index: Option<storage::pagetext::PageIndex>,
    /// Cards whose page text is due to be read for history, and when.
    pub(crate) text_due: HashMap<NodeId, Instant>,
    /// The last history search (query, results), as the palette asks often.
    pub(crate) visits_cache: RefCell<VisitsCache>,
    /// The palette's page preview, and a serial for its uploads.
    pub(crate) preview: Option<Preview>,
    pub(crate) preview_serial: u64,
    /// Page keys of sleeping cards already looked up for a thumbnail.
    pub(crate) thumb_tried: HashSet<u64>,
    /// Downloads (Ctrl+J), saved in `downloads.json`.
    pub(crate) downloads: scene::download::Downloads,
    /// Chromium's id of each running download → (list key, page key).
    pub(crate) download_ids: HashMap<u32, (u64, u64)>,
    /// Pages put to sleep or closed while downloading: kept alive (off
    /// screen, unrendered) until their downloads end, since Chromium only
    /// reports a download to the page that started it.
    pub(crate) parked: Vec<Page>,
    /// JavaScript, images and cookies per site (page menu, `:site`, `tj`…),
    /// saved in `sites.json`.
    pub(crate) sites: scene::sitesettings::SiteSettings,
    /// The same, for this run only (`:site-session`, `tJ`…); wins over `sites`.
    pub(crate) sites_session: scene::sitesettings::SiteSettings,
    /// The controls over the board, as laid out last frame (for the pointer).
    pub(crate) hud: crate::ui::hud::Hud,
    /// The control under the pointer, and since when (for its tooltip).
    pub(crate) hud_hover: Option<(crate::ui::hud::Control, Instant)>,
    /// The left button is held on a control.
    pub(crate) hud_down: bool,
    /// The control whose menu is open.
    pub(crate) hud_open: Option<crate::ui::hud::Control>,
    /// The size of a new card, shaped like the screen (`placement::card_size`).
    pub(crate) card_size: Vec2,
    /// What the pointer is over on the board (a label's buttons light up).
    pub(crate) hover: Hit,
    /// Where each card's label was drawn last frame (screen px), in draw
    /// order, and the buttons on them, for the pointer.
    pub(crate) bars: Vec<(NodeId, Rect)>,
    pub(crate) bar_buttons: Vec<(Hit, Rect)>,
    /// The buttons of the guidance drawn last frame (an empty board's
    /// welcome, a view with no card in it), and what each does.
    pub(crate) nudge: Vec<(Rect, crate::ui::hud::Go)>,
    /// The desktop asks for less motion (`desktop::reduced_motion`): cards,
    /// spaces, Solo and the sidebar change without animating.
    pub(crate) still: bool,
    /// Cards gliding to new places or growing in (`scene::motion`).
    pub(crate) glides: motion::Glides,
    /// Closed cards shrinking away: the card as it was, its page's texture
    /// (removed once it is gone), and since when.
    pub(crate) ghosts: Vec<(Node, u64, Instant)>,
    /// Trail lines just made by a join, drawing in from their source, and
    /// lines just cut by a leave, fading out; since when (M15).
    pub(crate) edges_in: Vec<(Edge, Instant)>,
    pub(crate) edges_out: Vec<(Edge, Instant)>,
    /// Cards of a trail being folded, each sliding onto its sheet behind the
    /// stack's front (`motion::folding`).
    pub(crate) gathers: Vec<Gather>,
    /// A space switch under way: since when, the side the board slides in
    /// from (±1), and the backdrop colours of the space it left.
    pub(crate) space_in: Option<(Instant, f32, [f32; 4], [f32; 4])>,
    /// The sidebar as last drawn and its right edge, to slide it out once
    /// it is hidden; and since when it has been sliding.
    pub(crate) sidebar_last: Option<(Layer, f32)>,
    pub(crate) sidebar_out: Option<Instant>,
    /// A download waiting for the user to say where it goes (`Target::SaveAs`).
    pub(crate) save_as: Option<SaveAs>,
    /// Sites asking for a permission, waiting for Enter (allow) or Esc; the
    /// first is the question showing (`Confirm::Permission`).
    pub(crate) asks: Vec<Ask>,
    /// Pages' own dialogs (alert, confirm, prompt, leave), shown after any
    /// permission question; the first is the one showing.
    pub(crate) dialogs: Vec<PageDialog>,
    /// A page's file chooser waiting for a path (`Target::Files`).
    pub(crate) choosing: Option<Choosing>,
    /// The folder last chosen from, where the next file chooser starts.
    pub(crate) files_dir: Option<std::path::PathBuf>,
    /// Ctrl+e in a field: waiting for its text, then for the editor.
    pub(crate) editing: Option<Editing>,
    /// The last token given to a field read (`web::field`).
    pub(crate) field_token: u64,
}

/// A site's permission request waiting for the user.
pub(crate) struct Ask {
    pub(crate) id: u64,
    pub(crate) origin: String,
    pub(crate) kinds: Vec<web::permission::Kind>,
}

/// Last history search (history view or palette, query, results).
pub(crate) type VisitsCache = Option<(bool, String, Vec<storage::pagetext::Visit>)>;

/// History pages matching `query`, cached per query: for the history view
/// (`browse`, newest first) or the palette (best first).
pub(crate) fn visits_for(
    index: &Option<storage::pagetext::PageIndex>,
    cache: &RefCell<VisitsCache>,
    query: &str,
    browse: bool,
) -> Vec<storage::pagetext::Visit> {
    let Some(index) = index else { return Vec::new() };
    if let Some((b, q, v)) = &*cache.borrow()
        && *b == browse
        && q == query
    {
        return v.clone();
    }
    let found = if browse {
        index.browse(query, palette::HISTORY_LIST)
    } else {
        // More than shown, as pages open in cards are left out.
        index.search(query, palette::MAX_HISTORY * 3)
    };
    let v = found.unwrap_or_else(|e| {
        log::warn!("history: searching {query:?}: {e}");
        Vec::new()
    });
    cache.replace(Some((browse, query.to_string(), v.clone())));
    v
}

/// Something that waits for Enter, as it can't be undone.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Confirm {
    DeleteSpace,
    /// Close every card but this one (co).
    CloseOthers(NodeId),
    ClearHistory,
    ClearCookies,
    /// The first of `App::asks`: Enter allows, anything else blocks.
    Permission,
    /// The first of `App::dialogs` (not a prompt, which is a field):
    /// Enter or y is OK, anything else Cancel.
    Dialog,
}

/// The desktop's locale (`en_IN.UTF-8`): `LC_ALL`, `LC_MESSAGES` or `LANG`.
pub(crate) fn system_locale() -> String {
    ["LC_ALL", "LC_MESSAGES", "LANG"].iter().find_map(|v| std::env::var(v).ok().filter(|l| !l.is_empty())).unwrap_or_default()
}

/// Local time in ms for a UTC time in ms (the zone's offset then).
pub(crate) fn local_ms(utc_ms: i64) -> i64 {
    use chrono::{Local, Offset, TimeZone};
    let offset = Local.timestamp_millis_opt(utc_ms).single().map_or(0, |d| d.offset().fix().local_minus_utc());
    utc_ms + i64::from(offset) * 1000
}

/// Opens a file or folder in the desktop's app for it (`xdg-open`, or
/// `TRAILBOARD_OPENER` for tests), without waiting for it.
pub(crate) fn open_externally(path: &str) {
    let opener = std::env::var("TRAILBOARD_OPENER").ok().filter(|o| !o.is_empty()).unwrap_or_else(|| "xdg-open".into());
    let path = path.to_string();
    std::thread::spawn(move || {
        let run = std::process::Command::new(&opener)
            .arg(&path)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .status();
        match run {
            Ok(s) if s.success() => {}
            Ok(s) => log::warn!("{opener} {path:?}: {s}"),
            Err(e) => log::warn!("{opener} {path:?}: {e}"),
        }
    });
}

pub(crate) fn now_ms() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis() as i64)
}

/// `path` with the home folder written as `~`.
/// The home folder (`$HOME`).
pub(crate) fn home_path() -> std::path::PathBuf {
    std::env::var_os("HOME").map(std::path::PathBuf::from).unwrap_or_default()
}

pub(crate) fn home_relative(path: &std::path::Path) -> String {
    match std::env::var_os("HOME").map(std::path::PathBuf::from) {
        Some(home) if home.as_os_str().len() > 1 => match path.strip_prefix(&home) {
            Ok(rest) => format!("~/{}", rest.display()),
            Err(_) => path.display().to_string(),
        },
        _ => path.display().to_string(),
    }
}

pub(crate) fn menu_labels(items: &[Entry]) -> Vec<&str> {
    items.iter().map(Entry::label).collect()
}

pub(crate) fn menu_layer(r: &mut Renderer, m: &ContextMenu, ui: f32, th: &Theme) -> Layer {
    let mut l = Layer::default();
    let radius = th.radius(ui / th.text_scale);
    let small = th.small_radius(ui / th.text_scale);
    crate::ui::chrome::panel(&mut l, m.rect, radius, th, ui, crate::ui::chrome::Lift::Mid, th.panel());
    let size = menu::FONT * ui;
    let ksize = crate::ui::chrome::type_::CAPTION * ui;
    for (i, (item, row)) in m.items.iter().zip(&m.rows).enumerate() {
        if item.is_separator() {
            let line = crate::ui::chrome::hair(ui);
            crate::ui::chrome::fill(&mut l, Rect::new(m.rect.x + line, (row.center().y - line / 2.0).round(), m.rect.w - 2.0 * line, line), 0.0, rgb(th.line(), 1.0));
            continue;
        }
        let hot = m.hover == Some(i);
        if hot {
            crate::ui::chrome::fill(&mut l, *row, small, rgb(th.active(), 1.0));
        }
        let mid = row.center().y;
        // The lead column: the row's icon, or a tick on the card in front
        // (or the space showing) for rows that have none.
        let icon_x = row.x + menu::ICON_X * ui;
        match item.icon() {
            Some(icon) => {
                let ink = match (item.is_enabled(), hot) {
                    (false, _) => th.text3(),
                    (true, true) => th.text(),
                    (true, false) => th.text2(),
                };
                crate::ui::icons::draw(&mut l, r, icon, icon_x, mid, 16.0 * ui, rgb(ink, 1.0));
            }
            None if item.is_current() => crate::ui::icons::draw(&mut l, r, crate::ui::icons::Icon::Check, icon_x, mid, 14.0 * ui, rgb(th.accent_text(), 1.0)),
            None => {}
        }
        let mut right = row.right() - 8.0 * ui;
        if let Some(keys) = crate::ui::chrome::keycaps(item.keys()) {
            let widths: Vec<f32> = keys.iter().map(|k| r.measure(k, ksize, false)).collect();
            right = crate::ui::chrome::draw_keycaps(&mut l, &keys, &widths, right, mid, ksize, th, ui, *row) - 12.0 * ui;
        }
        let x = row.x + (menu::TEXT_PAD - menu::PAD) * ui;
        let ink = match (item.is_enabled(), hot) {
            (false, _) => th.text3(),
            (true, true) => th.text(),
            (true, false) => crate::ui::theme::blend(th.text(), th.text2(), 0.35),
        };
        crate::ui::chrome::text(&mut l, item.label(), x, mid, size, rgb(ink, 1.0), Rect::new(x, row.y, (right - x).max(1.0), row.h), item.is_current());
    }
    l
}

/// Window cursor for a page cursor; `None` hides it.
pub(crate) fn cursor_icon(c: web::Cursor) -> Option<CursorIcon> {
    use web::Cursor as C;
    Some(match c {
        C::Default => CursorIcon::Default,
        C::Pointer => CursorIcon::Pointer,
        C::Text => CursorIcon::Text,
        C::VerticalText => CursorIcon::VerticalText,
        C::Wait => CursorIcon::Wait,
        C::Progress => CursorIcon::Progress,
        C::Help => CursorIcon::Help,
        C::Crosshair => CursorIcon::Crosshair,
        C::Cell => CursorIcon::Cell,
        C::Move => CursorIcon::Move,
        C::Grab => CursorIcon::Grab,
        C::Grabbing => CursorIcon::Grabbing,
        C::NotAllowed => CursorIcon::NotAllowed,
        C::NoDrop => CursorIcon::NoDrop,
        C::Copy => CursorIcon::Copy,
        C::Alias => CursorIcon::Alias,
        C::ContextMenu => CursorIcon::ContextMenu,
        C::ZoomIn => CursorIcon::ZoomIn,
        C::ZoomOut => CursorIcon::ZoomOut,
        C::AllScroll => CursorIcon::AllScroll,
        C::ResizeE => CursorIcon::EResize,
        C::ResizeN => CursorIcon::NResize,
        C::ResizeNE => CursorIcon::NeResize,
        C::ResizeNW => CursorIcon::NwResize,
        C::ResizeS => CursorIcon::SResize,
        C::ResizeSE => CursorIcon::SeResize,
        C::ResizeSW => CursorIcon::SwResize,
        C::ResizeW => CursorIcon::WResize,
        C::ResizeNS => CursorIcon::NsResize,
        C::ResizeEW => CursorIcon::EwResize,
        C::ResizeNESW => CursorIcon::NeswResize,
        C::ResizeNWSE => CursorIcon::NwseResize,
        C::ResizeCol => CursorIcon::ColResize,
        C::ResizeRow => CursorIcon::RowResize,
        C::Hidden => return None,
    })
}

/// `TRAILBOARD_MAX_LIVE`, if set: it overrides the *Pages kept awake* setting for this run.
pub(crate) fn max_live_from_env() -> Option<usize> {
    match std::env::var("TRAILBOARD_MAX_LIVE").ok().map(|v| v.trim().parse::<usize>()) {
        None => None,
        Some(Ok(n)) if n >= 1 => Some(n),
        Some(_) => {
            log::warn!("TRAILBOARD_MAX_LIVE must be a whole number ≥ 1; ignored");
            None
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum Hit {
    Board,
    /// A card's docked inspector, and the divider before it.
    Inspector(NodeId),
    Divider(NodeId),
    /// The title bar's buttons for a docked inspector (other side, own
    /// card) and an inspector card's (back into its page's card).
    DockSide(NodeId),
    Undock(NodeId),
    Redock(NodeId),
    Header(NodeId),
    Close(NodeId),
    /// The title bar's back, forward and reload buttons.
    Back(NodeId),
    Forward(NodeId),
    Reload(NodeId),
    Grip(NodeId),
    Content(NodeId),
    /// Solo's title bar.
    ReadBar(NodeId),
    /// A stack's count badge.
    Badge(NodeId),
    /// The blocked-request count on a card's title bar.
    Shield(NodeId),
    /// The title bar's awake button (cycles the card's mode) and its
    /// speaker (mutes the card's sound).
    Awake(NodeId),
    Mute(NodeId),
    /// A trail's tag above its first card on the board.
    Tag(NodeId),
}

impl Hit {
    pub(crate) fn id(&self) -> Option<NodeId> {
        match *self {
            Hit::Board => None,
            Hit::Header(id) | Hit::Close(id) | Hit::Back(id) | Hit::Forward(id) | Hit::Reload(id) | Hit::Grip(id) | Hit::Content(id) | Hit::ReadBar(id) | Hit::Badge(id) | Hit::Shield(id) | Hit::Tag(id) => Some(id),
            Hit::Inspector(id) | Hit::Divider(id) | Hit::DockSide(id) | Hit::Undock(id) | Hit::Redock(id) | Hit::Awake(id) | Hit::Mute(id) => Some(id),
        }
    }
}

