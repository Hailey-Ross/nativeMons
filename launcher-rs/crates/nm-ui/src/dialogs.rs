//! Dialogs: the C#'s FolderBrowserDialog, OpenFileDialog and the save-name box (AskText) as effects the platform
//! answers (SDL3's system pickers, a native prompt); the answer comes back through `dialog_done`. RunExport on top.

use crate::input::Effect;
use crate::launcher::Launcher;

pub type Then = Box<dyn FnOnce(&mut Launcher, Option<String>)>;

/// A WinForms filter ("Name|*.a;*.b|All files (*.*)|*.*") as (name, patterns without "*.") pairs, for SDL's pickers.
pub fn winforms_filter(f: &str) -> Vec<(String, String)> {
    let parts: Vec<&str> = f.split('|').collect();
    parts.chunks(2).filter(|c| c.len() == 2).map(|c| {
        let pats: Vec<String> = c[1].split(';').map(|p| p.trim().trim_start_matches("*.").to_string()).collect();
        (c[0].to_string(), if pats.iter().any(|p| p == "*") { "*".to_string() } else { pats.join(";") })
    }).collect()
}

impl Launcher {
    fn ask(&mut self, e: Effect, then: Then) {
        // A replay answers from its `answer` lines (none left: Cancel), noted as the C# replay notes it.
        if self.hooks.replay {
            let what = match &e { Effect::PickFolder { title } => format!("PickFolder: {}", title), Effect::PickFile { title, .. } => format!("OpenFile: {}", title), _ => String::new() };
            self.effects.push(Effect::Dialog(what));
            let a = self.jobs.answers.pop_front().flatten();
            then(self, a);
            self.dirty = true;
            return;
        }
        // One at a time: a dialog is modal in the C#.
        if self.jobs.dialog.is_some() { return; }
        self.jobs.dialog = Some(then);
        self.effects.push(e);
    }
    /// PickFolder(prompt).
    pub fn pick_folder(&mut self, prompt: &str, then: Then) { self.ask(Effect::PickFolder { title: prompt.into() }, then); }
    /// OpenFileDialog { Filter, Title }.
    pub fn pick_file(&mut self, title: &str, filter: &str, then: Then) { self.ask(Effect::PickFile { title: title.into(), filters: winforms_filter(filter) }, then); }
    /// AskText(title, prompt, start): the answer with line breaks as spaces, trimmed; None when canceled.
    pub fn ask_text(&mut self, title: &str, prompt: &str, start: &str, then: Then) {
        // The C# replay notes the box instead of showing it.
        if self.hooks.replay { self.todo(&format!("AskText: {}", title)); return; }
        self.ask(Effect::AskText { title: title.into(), prompt: prompt.into(), start: start.into() }, Box::new(|l, v| then(l, v.map(|s| s.replace(['\r', '\n'], " ").trim().to_string()))));
    }
    /// The platform's answer to the open dialog (None: canceled or closed).
    pub fn dialog_done(&mut self, answer: Option<String>) {
        if let Some(t) = self.jobs.dialog.take() { t(self, answer); }
        self.dirty = true;
    }
    /// SaveAction: the work on a Busy card with the saves list read again, then back `pop` pages, the fresh list, and
    /// what went wrong if anything did.
    pub fn save_action(&mut self, id: &str, title: &str, work: impl FnOnce(&[nm_core::game_entry::GameEntry], &nm_core::game_entry::GameEntry) -> Option<String> + Send + 'static, pop: usize) {
        if self.hooks.replay { self.todo(&format!("SaveAction: {}", title)); return; }
        let Some(g) = self.game(id) else { return };
        let games = self.ctx().lib.games.clone();
        let id = id.to_string();
        // Pages hold UI-thread pointers, so the fresh list is read on the UI thread after the work (the C# reads it on
        // the worker).
        self.busy(title, move || Ok(work(&games, &g)), move |l, r| {
            let problem = match r { Ok(p) => p, Err(e) => Some(e) };
            let fresh = l.game(&id).and_then(|g| crate::pages::saves_page(&g));
            for _ in 0..pop { if l.pages.len() > 1 { l.pages.pop(); } }
            if let Some(f) = fresh { l.push(f); }
            l.replace_main();
            if let Some(p) = problem { l.show_text("That didn't work", &p); }
        });
    }
    /// RunExport: where the export goes, then the copy on a Busy card, then what happened.
    pub fn run_export(&mut self, id: &str, checkpoint: bool, backup: Option<String>) {
        if self.hooks.replay { self.todo(&format!("RunExport: {}", if checkpoint { "checkpoint" } else { "plain" })); return; }
        let id = id.to_string();
        self.pick_folder("Where should the export go?", Box::new(move |l, out| {
            let (Some(out), Some(g)) = (out, l.game(&id)) else { return };
            let title = g.title.clone();
            l.busy(&format!("Exporting {}", title), move || Ok(nm_core::saves::export_save(&g, checkpoint, &out, backup.as_deref())), |l, r| {
                match r { Ok((m, ok)) => l.show_text(if ok { "Exported" } else { "Not exported" }, &m), Err(e) => l.show_text("Not exported", &e) }
            });
        }));
    }
    /// The third export item: a Checkpoint backup chosen first (the C# asks before RunExport).
    pub fn run_export_from_backup(&mut self, id: &str) {
        if self.hooks.replay { self.todo("RunExport: PickFolder"); return; }
        let Some(g) = self.game(id) else { return };
        let id = id.to_string();
        self.pick_folder(&format!("Choose a Checkpoint backup of {} from your 3DS", g.title), Box::new(move |l, backup| l.run_export(&id, true, backup)));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn filters() {
        assert_eq!(winforms_filter("Cartridge save (*.sav)|*.sav|All files (*.*)|*.*"), vec![("Cartridge save (*.sav)".to_string(), "sav".to_string()), ("All files (*.*)".to_string(), "*".to_string())]);
        assert_eq!(winforms_filter("Pictures (*.png, *.jpg)|*.png;*.jpg;*.jpeg"), vec![("Pictures (*.png, *.jpg)".to_string(), "png;jpg;jpeg".to_string())]);
        assert_eq!(winforms_filter("OpenHome|OpenHome*.exe|Programs (*.exe)|*.exe")[0].1, "OpenHome*.exe");
    }
}
