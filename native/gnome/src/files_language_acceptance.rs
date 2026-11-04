//! Live Files language, literal model data and reused row-property ownership.
use super::*;
use std::path::Path;
struct Row {
    data: Data,
    root: bool,
    icon: gtk::Image,
    label: gtk::Label,
    size: gtk::Label,
    english: String,
    chinese: String,
}
impl Row {
    fn render(&self) {
        rows::present(&self.data, self.root, &self.icon, &self.label, &self.size);
    }
}
fn entry(name: &str, path: &str, read_only: bool) -> Data {
    Data::Entry(workspace::Entry {
        name: name.into(),
        path: path.into(),
        kind: "file".into(),
        size: None,
        read_only,
    })
}
impl Files {
    pub(super) async fn smoke_language(&self, directory: &Path) -> Result<(), String> {
        struct Restore<'a> {
            files: &'a Files,
            window: adw::Window,
            chinese: bool,
        }
        impl Drop for Restore<'_> {
            fn drop(&mut self) {
                self.files.close_context();
                self.window.close();
                let _ = crate::language::set(if self.chinese { "zh-CN" } else { "en-US" }, "en-US");
                i18n_ui::apply();
            }
        }
        let content = gtk::Box::new(gtk::Orientation::Vertical, 8);
        content.set_margin_start(16);
        content.set_margin_end(16);
        content.set_margin_top(16);
        content.set_margin_bottom(16);
        let window = adw::Window::builder()
            .title("Private Files language check")
            .transient_for(&self.window)
            .default_width(700)
            .content(&content)
            .build();
        let _restore = Restore {
            files: self,
            window: window.clone(),
            chinese: crate::language::tr("Files") == "文件",
        };
        let error = "Send {count} <b>Files</b> 中文😀 ";
        let mut cases = vec![
            (
                entry("Project", "", false),
                true,
                "Project".to_string(),
                "项目".to_string(),
            ),
            (
                entry(
                    "Shared · Send {count} 中文😀",
                    "/private/Send {count} 中文😀",
                    true,
                ),
                true,
                "Shared · Send {count} 中文😀".into(),
                "共享 · Send {count} 中文😀".into(),
            ),
            (
                Data::More {
                    parent: String::new(),
                    cursor: "Files {count}".into(),
                },
                false,
                "Load more files".into(),
                "加载更多文件".into(),
            ),
            (
                Data::Notice {
                    parent: String::new(),
                    message: "Empty folder",
                    error: None,
                    retry: false,
                },
                false,
                "Empty folder".into(),
                "空文件夹".into(),
            ),
            (
                Data::Notice {
                    parent: String::new(),
                    message: "{error} · Retry",
                    error: Some(error.into()),
                    retry: true,
                },
                false,
                format!("{error} · Retry"),
                format!("{error} · 重试"),
            ),
            (
                Data::Notice {
                    parent: String::new(),
                    message: "Close some folders before expanding more (maximum 64)",
                    error: None,
                    retry: true,
                },
                false,
                "Close some folders before expanding more (maximum 64)".into(),
                "请先收起部分文件夹再继续展开（最多 64 个）".into(),
            ),
        ];
        for name in [
            "Project",
            "Files",
            "Read only",
            "Load more files",
            "Shared · Send",
            "中文😀 {count} ",
        ] {
            cases.push((
                entry(name, name, name == "Read only"),
                false,
                name.into(),
                name.into(),
            ));
        }
        let mut sized = entry("Size", "/private/Size", true);
        if let Data::Entry(value) = &mut sized {
            value.size = Some(1024);
        }
        cases.push((sized, false, "Size".into(), "Size".into()));
        let mut rows = Vec::new();
        for (data, root, english, chinese) in cases {
            let row = Row {
                data,
                root,
                icon: gtk::Image::new(),
                label: gtk::Label::new(None),
                size: gtk::Label::new(None),
                english,
                chinese,
            };
            row.label.set_xalign(0.0);
            row.label.set_hexpand(true);
            let line = gtk::Box::new(gtk::Orientation::Horizontal, 12);
            line.append(&row.icon);
            line.append(&row.label);
            line.append(&row.size);
            content.append(&line);
            row.render();
            rows.push(row);
        }
        crate::window_capture::observe_window(&window);
        // The real Files controls, model and menu remain alive across changes.
        self.context_menu("nested/note.md", 16.0, 16.0);
        let popup = self
            .context
            .borrow()
            .clone()
            .ok_or("Files context missing")?;
        let menu = popup.menu_model().ok_or("Files menu model missing")?;
        let targets = (0..menu.n_items())
            .map(|i| menu.item_attribute_value(i, "target", None))
            .collect::<Vec<_>>();
        let before = (
            self.generation.get(),
            self.root.borrow().clone(),
            self.search.text(),
            self.hidden.is_active(),
            self.ignored.is_active(),
            self.expanded(),
        );
        for (locale, chinese) in [("zh-CN", true), ("en-US", false), ("zh-CN", true)] {
            crate::language::set(locale, "en-US")?;
            i18n_ui::apply();
            crate::window_capture::next_frame(&self.widget).await?;
            for row in &rows {
                let expected = if chinese { &row.chinese } else { &row.english };
                if row.label.text().as_str() != expected || row.label.uses_markup() {
                    return Err(format!(
                        "Files locale changed row content: expected {expected:?}, got {:?}",
                        row.label.text()
                    ));
                }
            }
            if self.hidden.label().as_deref()
                != Some(if chinese {
                    "显示隐藏文件"
                } else {
                    "Show hidden files"
                })
                || self.ignored.label().as_deref()
                    != Some(if chinese {
                        "显示已忽略的文件"
                    } else {
                        "Show ignored files"
                    })
                || self.search.placeholder_text().as_deref()
                    != Some(if chinese {
                        "筛选已展开的文件夹"
                    } else {
                        "Filter expanded folders"
                    })
                || self.refresh.tooltip_text().as_deref()
                    != Some(if chinese {
                        "刷新已展开的文件夹"
                    } else {
                        "Refresh expanded folders"
                    })
                || rows
                    .iter()
                    .find(|r| matches!(&r.data, Data::Entry(e) if e.path == "Read only"))
                    .unwrap()
                    .size
                    .text()
                    != if chinese { "只读" } else { "Read only" }
                || rows.last().unwrap().size.text()
                    != format!(
                        "{} · {}",
                        glib::format_size(1024),
                        if chinese { "只读" } else { "Read only" }
                    )
                || !popup.is_mapped()
            {
                return Err("Files controls/badges did not follow the live language".into());
            }
            for (i, expected) in [
                (
                    0,
                    if chinese {
                        "在文件管理器中显示"
                    } else {
                        "Show in Files"
                    },
                ),
                (1, if chinese { "复制路径" } else { "Copy path" }),
            ] {
                if menu
                    .item_attribute_value(i, "label", None)
                    .and_then(|v| v.str().map(str::to_string))
                    .as_deref()
                    != Some(expected)
                    || menu.item_attribute_value(i, "target", None) != targets[i as usize]
                {
                    return Err(
                        "Files menu translation changed its literal target or lost its label"
                            .into(),
                    );
                }
            }
            if before
                != (
                    self.generation.get(),
                    self.root.borrow().clone(),
                    self.search.text(),
                    self.hidden.is_active(),
                    self.ignored.is_active(),
                    self.expanded(),
                )
            {
                return Err("Files language reset navigation, query or visibility state".into());
            }
        }
        self.close_context();
        window.present();
        crate::window_capture::ready(&content).await?;
        crate::window_capture::capture(&window, &directory.join("files-language-chinese.png"))
            .await?;
        // Reuse exactly the same production row widgets after a notice. The new
        // filename equals the last translated label; value equality cannot
        // determine whether a binding still owns it.
        let recycled = &rows[2];
        rows::present(
            &entry("加载更多文件", "Files", false),
            false,
            &recycled.icon,
            &recycled.label,
            &recycled.size,
        );
        let diagnostic = &rows[4];
        rows::present(
            &entry("Send", "Empty folder", false),
            false,
            &diagnostic.icon,
            &diagnostic.label,
            &diagnostic.size,
        );
        for locale in ["en-US", "zh-CN"] {
            crate::language::set(locale, "en-US")?;
            i18n_ui::apply();
            if recycled.label.text() != "加载更多文件"
                || recycled.label.tooltip_text().as_deref() != Some("Files")
                || !recycled.size.text().is_empty()
                || diagnostic.label.text() != "Send"
                || diagnostic.label.tooltip_text().as_deref() != Some("Empty folder")
            {
                return Err("A recycled Files row retained a previous interface binding".into());
            }
        }
        Ok(())
    }
}
