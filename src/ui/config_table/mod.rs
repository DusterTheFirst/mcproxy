use std::{
    collections::HashMap,
    convert::identity,
    fmt::{Display, Write},
    sync::Arc,
};

use crate::{
    config::schema::{
        Config, PlaceholderServerConfig, PlaceholderServerResponses, ProxyConfig, UiServerConfig,
    },
    proto::{
        packet::TextComponentObject,
        response::{Player, Players, StatusResponse, Version},
    },
};

struct Unindenter<W>(W);

impl<W> Unindenter<W> {
    pub fn into_inner(self) -> W {
        self.0
    }
}

impl<W: Write> Write for Unindenter<W> {
    fn write_str(&mut self, s: &str) -> std::fmt::Result {
        #[derive(Debug)]
        enum State {
            Text { start: usize },
            Indentation,
        }

        let mut state = State::Text { start: 0 };

        for (i, char) in s.char_indices() {
            match state {
                State::Text { start } => {
                    if char == '\n' {
                        self.0.write_str(&s[start..=i])?;
                        state = State::Indentation;
                    } else {
                        state = State::Text { start }
                    }
                }
                State::Indentation => {
                    if !char.is_whitespace() && char != '\n' {
                        state = State::Text { start: i };
                    }
                }
            }
        }

        if let State::Text { start } = state {
            self.0.write_str(&s[start..])?;
        }

        Ok(())
    }
}

pub fn config_table(config: Arc<Config>) -> String {
    let mut html = Unindenter(String::new());

    write!(
        html,
        r#"<!DOCTYPE html>
        <html lang="en">
        <head>
            <meta charset="UTF-8">
            <meta name="viewport" content="width=device-width, initial-scale=1.0">
            <style>{}</style>
        </head>
        <body>
        <table>
            <caption>Configuration values</caption>
            <thead>
                <tr>
                    <th>table</th>
                    <th>value</th>
                </tr>
            </thead>
            <tbody>"#,
        include_str!("./style.css")
    )
    .unwrap();

    {
        let Config {
            placeholder_server,
            static_servers,
            ui,
            proxy,
        } = config.as_ref();

        if let Some(UiServerConfig { listen_address }) = ui {
            config_value(&mut html, &"ui.listen_address", &|w| {
                write!(w, "{listen_address}").unwrap()
            });
        }

        {
            let ProxyConfig { listen_address } = proxy;
            config_value(&mut html, &"proxy.listen_address", &|w| {
                write!(w, "{listen_address}").unwrap()
            });
        }

        config_value(&mut html, &"static_servers", &|w| {
            kv_mapping(w, static_servers);
        });

        {
            let PlaceholderServerConfig { responses } = placeholder_server;

            {
                let PlaceholderServerResponses {
                    offline,
                    no_mapping,
                } = responses;

                config_value(&mut html, &"placeholder_server.responses", &|w| {
                    table(w, None, &|w| {
                        for (response_name, response) in
                            [("offline", offline), ("no_mapping", no_mapping)]
                        {
                            config_value(w, &response_name, &|w| {
                                if let Some(StatusResponse {
                                    version,
                                    players,
                                    description,
                                    favicon,
                                }) = response
                                {
                                    table(w, None, &|w| {
                                        if let Some(favicon) = favicon {
                                            config_value(w, &"favicon", &|w| {
                                                write!(w, r#"<img src="{favicon}"/>"#).unwrap();
                                            });
                                        }

                                        config_value(w, &"version", &|w| {
                                            table(w, None, &|w| {
                                                let Version { name, protocol } = version;

                                                config_value(w, &"name", &|w| {
                                                    write!(w, "{name}").unwrap()
                                                });
                                                config_value(w, &"protocol", &|w| {
                                                    write!(w, "{protocol}").unwrap()
                                                });
                                            });
                                        });

                                        config_value(w, &"players", &|w| {
                                            table(w, None, &|w| {
                                                let Some(Players {
                                                    max,
                                                    online,
                                                    sample,
                                                }) = players
                                                else {
                                                    return;
                                                };

                                                config_value(w, &"max", &|w| {
                                                    write!(w, "{max}").unwrap()
                                                });
                                                config_value(w, &"online", &|w| {
                                                    write!(w, "{online}").unwrap()
                                                });

                                                config_value(w, &"sample", &|w| {
                                                    table(w, None, &|w| {
                                                        for Player { name, id } in sample {
                                                            tr_td(w, &|w| {
                                                                table(w, None, &|w| {
                                                                    config_value(
                                                                        w,
                                                                        &"name",
                                                                        &|w| {
                                                                            write!(w, "{name}")
                                                                                .unwrap()
                                                                        },
                                                                    );
                                                                    config_value(w, &"id", &|w| {
                                                                        write!(w, "{id}").unwrap()
                                                                    });
                                                                });
                                                            });
                                                        }
                                                    });
                                                });
                                            });
                                        });

                                        config_value(w, &"description", &|w| {
                                            write!(w, "<pre><code class=\"mc-font\">").unwrap();
                                            text_component_html(
                                                w,
                                                TextComponentObject::from(description.clone()),
                                            );
                                            write!(w, "</code></pre>").unwrap();
                                        });
                                    })
                                }
                            });
                        }
                    });
                })
            }
        }
    }

    write!(html, r#"</tbody></table></body></html>"#).unwrap();

    html.into_inner()
}

fn text_component_html(w: &mut dyn Write, component: TextComponentObject) {
    let TextComponentObject {
        text,
        bold,
        italic,
        underlined,
        strikethrough,
        obfuscated,
        color,
        extra,
    } = component;

    write!(w, "<span").unwrap();

    let bold = bold.is_some_and(identity);
    let italic = italic.is_some_and(identity);
    let underlined = underlined.is_some_and(identity);
    let strikethrough = strikethrough.is_some_and(identity);

    if bold || italic || underlined || strikethrough || color.is_some() {
        write!(w, " style=\"").unwrap();
        if bold {
            write!(w, "font-weight:bold;").unwrap();
        }
        if italic {
            write!(w, "font-style:italic;").unwrap();
        }
        if underlined || strikethrough {
            write!(w, "text-decoration-line:").unwrap();
            if underlined {
                write!(w, "underline").unwrap();
            }
            if strikethrough {
                write!(w, " line-through").unwrap();
            }
            write!(w, ";").unwrap();
        }
        if let Some(color) = color.as_ref().map(|c| c.foreground_color()) {
            write!(w, "color:{color};").unwrap();
        }
        write!(w, "\"").unwrap();
    }

    if obfuscated.is_some_and(identity) {
        write!(w, " class=\"obfuscated\"").unwrap();
    }

    write!(w, ">{text}").unwrap();
    for component in extra.unwrap_or_default() {
        text_component_html(w, component.into());
    }
    write!(w, "</span>").unwrap();
}

fn tr_td(w: &mut dyn Write, inner: &dyn Fn(&mut dyn Write)) {
    write!(w, "<tr><td>").unwrap();
    inner(w);
    write!(w, "</td></tr>").unwrap();
}

fn table(
    w: &mut dyn Write,
    inner_head: Option<&dyn Fn(&mut dyn Write)>,
    inner_body: &dyn Fn(&mut dyn Write),
) {
    write!(w, "<table>").unwrap();
    if let Some(inner_head) = inner_head {
        write!(w, "<thead>").unwrap();
        inner_head(w);
        write!(w, "</thead>").unwrap();
    }

    write!(w, "<tbody>").unwrap();
    inner_body(w);
    write!(w, "</tbody></table>").unwrap();
}

fn config_value(w: &mut dyn Write, name: &dyn Display, inner: &dyn Fn(&mut dyn Write)) {
    write!(
        w,
        r#"<tr><th scope="row"><pre><code>{name}</code></pre></th><td>"#
    )
    .unwrap();

    inner(w);

    write!(w, r#"</td></tr>"#).unwrap();
}

fn kv_mapping(w: &mut dyn Write, map: &HashMap<impl Display + Ord, impl Display>) {
    write!(w, r#"<table>"#).unwrap();

    let mut pairs = map.iter().collect::<Vec<_>>();
    pairs.sort_by(|(k_a, _), (k_b, _)| k_a.cmp(k_b));

    for (key, value) in pairs {
        write!(w, r#"<tr><th scope="row">{key}</th><td>{value}</td></tr>"#).unwrap();
    }

    write!(w, r#"</table>"#).unwrap();
}
