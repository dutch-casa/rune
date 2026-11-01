//! The button that opens a pane's narrow, widen and reset menu.

use rune::{El, Transition, icon};

use super::super::pane::{PaneState, StepMenu};
use super::{Apply, INSPECTOR_WIDTH, KEY_STEP, SIDEBAR_WIDTH};
use crate::{ControlSize, IconButton, IconButtonVariant, Kit, Menu, MenuItem};

pub(super) fn pane_step_menu<S: 'static>(
    anchor: StepMenu,
    access: fn(&mut S) -> &mut PaneState,
    sign: f32,
    key: &'static str,
    control: &'static str,
) -> Box<dyn FnOnce(&Kit, &mut Vec<El>)> {
    Box::new(move |k, layer| {
        let glyph = |turn: f32| icon(k.glyph.chevron_down, 14.0).icon_rotation(turn);
        let right = -std::f32::consts::FRAC_PI_2;
        let left = std::f32::consts::FRAC_PI_2;
        let (narrow_turn, widen_turn) = if sign > 0.0 { (left, right) } else { (right, left) };
        let step = {
            let access = access;
            move |delta: f32| {
                let access = access;
                move |s: &mut S| {
                    let p = access(s);
                    if p.is_open() {
                        p.set_width(p.width() + sign * delta * KEY_STEP);
                    }
                    p.set_step_menu(None);
                }
            }
        };
        let menu = Menu::new(key)
            .open(true)
            .item(MenuItem::new("Narrow").lead(glyph(narrow_turn)).on_select(step(-sign)))
            .item(MenuItem::new("Widen").lead(glyph(widen_turn)).on_select(step(sign)))
            .item(MenuItem::new("Reset").on_select({
                let access = access;
                move |s: &mut S| {
                    let p = access(s);
                    p.reset_width();
                    p.set_step_menu(None);
                }
            }))
            .on_close({
                let access = access;
                move |s: &mut S| access(s).set_step_menu(None)
            });
        let menu = match anchor {
            StepMenu::At(point) => menu.at(point.x, point.y),
            StepMenu::Control => menu.beside(control),
        };
        let _ = menu.build(k, layer);
    })
}

pub(super) fn width_control(k: &Kit, leading: bool, bind: Apply) -> El {
    let button = IconButton::new(k.glyph.chevron_down)
        .variant(IconButtonVariant::GhostMuted)
        .control_size(ControlSize::Regular)
        .label(if leading { "Sidebar width" } else { "Inspector width" })
        .help("Narrow, widen, or reset")
        .build(k)
        .key(if leading { SIDEBAR_WIDTH } else { INSPECTOR_WIDTH })
        .transition(Transition::none());
    bind(button)
}
