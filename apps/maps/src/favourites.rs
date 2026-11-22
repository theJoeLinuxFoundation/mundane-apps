use super::*;

impl Maps {
    fn marked_or_center(&self) -> Place {
        self.place.clone().unwrap_or_else(|| Place {
            display_name: format!("{:.4}, {:.4}", self.camera.lat, self.camera.lon),
            lat: self.camera.lat.to_string(),
            lon: self.camera.lon.to_string(),
        })
    }
    pub(super) fn refresh_favourites(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        match self.source.favourites() {
            Ok(list) => self.favourites = list,
            Err(e) => self.status = format!("Favourites unavailable · {e}"),
        }
        let saved = self
            .favourites
            .iter()
            .any(|p| p.point() == self.marked_or_center().point());
        let mut items = vec![
            mundane_ui::MenuChoice::new("here", "Current location"),
            mundane_ui::MenuChoice::new(
                "save",
                if saved {
                    "Remove from favourites"
                } else {
                    "Add to favourites"
                },
            ),
        ];
        items.extend(
            self.favourites
                .iter()
                .enumerate()
                .map(|(i, p)| mundane_ui::MenuChoice::new(i.to_string(), p.display_name.clone())),
        );
        self.saved_places
            .update(cx, |m, cx| m.replace(items, w, cx));
    }
    pub(super) fn centre(
        &mut self,
        place: Place,
        lat: f64,
        lon: f64,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.camera = Camera {
            lat,
            lon,
            zoom: 12u8.clamp(self.range.0, self.range.1),
        };
        self.place = Some(place);
        self.show_results = false;
        self.focus.focus(w, cx);
        self.refresh_favourites(w, cx);
        self.draw(false, cx);
    }
    pub(super) fn toggle_favourite(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        self.status = match self.source.toggle_favourite(&self.marked_or_center()) {
            Ok(true) => "Added to favourites".into(),
            Ok(false) => "Removed from favourites".into(),
            Err(e) => format!("Could not save favourite · {e}"),
        };
        self.refresh_favourites(w, cx);
        cx.notify();
    }
    pub(super) fn place_action(&mut self, key: &str, w: &mut Window, cx: &mut Context<Self>) {
        match key {
            "save" => self.toggle_favourite(w, cx),
            "here" => self.locate(w, cx),
            _ => {
                if let Some(p) = key
                    .parse::<usize>()
                    .ok()
                    .and_then(|i| self.favourites.get(i))
                    .cloned()
                    && let Some((lat, lon)) = p.point()
                {
                    self.centre(p, lat, lon, w, cx)
                }
            }
        }
    }
    pub(super) fn locate(&mut self, w: &mut Window, cx: &mut Context<Self>) {
        if self.locating {
            return;
        }
        self.locating = true;
        self.show_results = false;
        self.status = "Waiting for location permission…".into();
        cx.notify();
        cx.spawn_in(w, async move |view, cx| {
            let result = mundane_services::location::locate(false).await;
            let _ = view.update_in(cx, |v, w, cx| {
                v.locating = false;
                match result {
                    Ok((lat, lon)) => v.centre(
                        Place {
                            display_name: "Current location".into(),
                            lat: lat.to_string(),
                            lon: lon.to_string(),
                        },
                        lat,
                        lon,
                        w,
                        cx,
                    ),
                    Err(e) => {
                        v.status = format!("Location unavailable · {e}");
                        cx.notify();
                    }
                }
            });
        })
        .detach();
    }
}
