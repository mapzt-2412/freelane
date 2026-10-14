use super::*;
impl Desktop {
    pub fn discover_apps(&mut self, ctx: &egui::Context) {
        let Some(vm) = &self.vm else {
            return;
        };
        let path = vm.path.clone();
        self.work(ctx, move || {
            let result = (|| -> Result<Event, String> {
                let info =
                    freelane::agent_capabilities::inspect(&path).map_err(|e| e.to_string())?;
                if info.supports("app_catalog", freelane_agent_protocol::CATALOG_VERSION)
                    != Some(true)
                {
                    return Ok(Event::Notice(
                        "Update guest agent for application discovery".into(),
                    ));
                }
                let mut cursor = 0;
                let mut entries = Vec::new();
                loop {
                    let response =
                        freelane::agent::app_catalog(&path, cursor).map_err(|e| e.to_string())?;
                    let Outcome::AppCatalog { page } = response.outcome else {
                        return Err("Application catalog unavailable".into());
                    };
                    entries.extend(page.apps);
                    if entries.len() > 512 {
                        return Err("Application catalog exceeds limit".into());
                    }
                    match page.next {
                        Some(next) => cursor = next,
                        None => break,
                    }
                }
                Ok(Event::AppCatalog(path, entries))
            })();
            Ok(result.unwrap_or_else(Event::Notice))
        });
    }
}
