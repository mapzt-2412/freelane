use freelane_guest_agent::catalog::{page, Entry};
#[test]
fn catalog_pages_are_bounded_deterministic_and_do_not_parse_shell_arguments() {
    let entries = (0..30)
        .map(|n| Entry {
            name: format!("App {n:02}"),
            executable: format!("/usr/bin/app{n:02}"),
        })
        .collect::<Vec<_>>();
    let first = page(entries.clone(), 0).unwrap();
    assert_eq!(first.apps.len(), 12);
    assert_eq!(first.next, Some(12));
    let second = page(entries, 12).unwrap();
    assert_eq!(second.apps[0].name, "App 12");
    assert_eq!(second.next, Some(24));
    assert!(page(
        vec![Entry {
            name: "bad".into(),
            executable: "sh -c arbitrary".into()
        }],
        0
    )
    .is_err());
}

#[test]
fn aliases_are_deduplicated_even_when_display_names_sort_apart() {
    let result = page(
        vec![
            Entry {
                name: "A".into(),
                executable: "/bin/same".into(),
            },
            Entry {
                name: "B".into(),
                executable: "/bin/other".into(),
            },
            Entry {
                name: "Z".into(),
                executable: "/bin/same".into(),
            },
        ],
        0,
    )
    .unwrap();
    assert_eq!(result.apps.len(), 2);
    assert_eq!(result.next, None);
}
