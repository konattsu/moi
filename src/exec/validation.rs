#[derive(Clone)]
enum Scope {
    Directory,
    WholeTree,
    File,
    Block(String),
}

pub(crate) fn validate(
    operations: &[crate::exec::standard::Prepared<'_>],
) -> Result<(), crate::error::MoiError> {
    let mut claims = Vec::new();
    for operation in operations.iter().filter(|o| o.applicable) {
        let Some(dst) = &operation.dst else {
            continue;
        };
        match operation.operation {
            crate::model::Operation::Archive(a) => {
                if let Some(include) = a.selected_paths()? {
                    for relative in include {
                        claims.push((
                            dst.join(relative),
                            Scope::File,
                            operation.label(),
                        ));
                    }
                } else {
                    claims.push((dst.clone(), Scope::WholeTree, operation.label()));
                }
            }
            crate::model::Operation::Directory { .. } => {
                claims.push((dst.clone(), Scope::Directory, operation.label()))
            }
            crate::model::Operation::Block { marker, .. } => claims.push((
                dst.clone(),
                Scope::Block(marker.clone()),
                operation.label(),
            )),
            _ => claims.push((dst.clone(), Scope::File, operation.label())),
        }
    }
    for (i, (a, sa, la)) in claims.iter().enumerate() {
        for (b, sb, lb) in &claims[i + 1..] {
            let conflict = if a == b {
                !matches!((sa,sb),(Scope::Block(ma),Scope::Block(mb)) if ma != mb)
            } else if b.starts_with(a) {
                !matches!(sa, Scope::Directory)
            } else if a.starts_with(b) {
                !matches!(sb, Scope::Directory)
            } else {
                false
            };
            if conflict {
                return Err(crate::error::MoiError::config(format!(
                    "destination conflict: {la} <-> {lb}"
                )));
            }
        }
    }
    Ok(())
}
