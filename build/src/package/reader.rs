use anyhow::Result;
use std::{path::PathBuf, str::FromStr};
use tracing::{debug, instrument, trace, warn};

use crate::{
    icon::{Category, IconSize, SvgIcon},
    package::{Package, PackageType},
};

use super::Unknown;

/// A directory to be searched, combined with:
///     - a list of categories valid for the contents of that directory and
///     - an icon size valid for icons inside the directory
#[derive(Debug)]
struct SearchDir {
    path: PathBuf,
    categories: Vec<Category>,
    icon_size: Option<IconSize>,
}

#[instrument(level = "info", skip(package), fields(package = ?package.ty))]
pub(crate) async fn read_icons(
    package: &Package<Unknown>,
    icons_path: PathBuf,
) -> Result<Vec<SvgIcon>> {
    trace!("Reading icon data...");
    let mut icons = Vec::new();

    let mut search_dirs = Vec::<SearchDir>::new();

    search_dirs.push(SearchDir {
        path: icons_path,
        categories: Vec::new(),
        icon_size: None,
    });

    while let Some(SearchDir {
        path,
        categories,
        icon_size,
    }) = search_dirs.pop()
    {
        let mut dir_stream = tokio::fs::read_dir(&path).await?;
        while let Some(entry) = dir_stream.next_entry().await? {
            let entry_path = entry.path();

            // We found a nested directory which should also be searched.
            if entry.file_type().await?.is_dir() {
                debug!(additional_dir = ?entry_path, "Found additional directory.");

                let file_name = entry_path
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
                    .to_string();

                // The first directory being parsable as an IconSize counts.
                let icon_size = icon_size.or_else(|| IconSize::from_str(&file_name).ok());

                // The new directory needs all categories from the current directory.
                // We may consider the dir name as being a "category" for all items contained in it.
                let mut entry_cats = categories.clone();
                if package.ty.is_category(&file_name) {
                    entry_cats.push(Category(file_name));
                }

                search_dirs.push(SearchDir {
                    path: path.join(&entry_path),
                    categories: entry_cats,
                    icon_size,
                });

                continue;
            };

            // We found a file and can check its extension.
            match entry_path.extension() {
                Some(file_extension) => match file_extension.to_str() {
                    Some(file_extension) => match file_extension {
                        "svg" => {
                            if package.ty == PackageType::FluentUISystemIcons {
                                if categories.len() != 2 {
                                    continue;
                                }
                                if categories.iter().any(|cat| {
                                    cat.0.ends_with("Temp LTR") || cat.0.ends_with("Temp RTL")
                                }) {
                                    continue;
                                }
                                let file_stem = entry_path.file_stem().unwrap().to_string_lossy();
                                if !(file_stem.ends_with("20_regular")
                                    || file_stem.ends_with("20_filled"))
                                {
                                    continue;
                                }
                            }

                            icons.push(
                                SvgIcon::new(package, &entry_path, icon_size, categories.clone())
                                    .await?,
                            );
                        }
                        _ => trace!(
                            ?entry_path,
                            file_extension,
                            "Found file without svg extension. Ignoring it."
                        ),
                    },
                    None => {
                        warn!(
                                ?entry_path,
                                ?file_extension,
                                "Found file whose file_extension (&OsStr) could not be converted to a &str. Ignoring it."
                            );
                    }
                },
                None => warn!(?entry_path, "Found file without extension. Ignoring it."),
            };
        }
    }

    trace!(num_icons = icons.len(), "Finished retrieving icon names.");
    Ok(icons)
}
