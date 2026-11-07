//! Folders over the wire: make, rename, move, delete, resolve, list; the disk follows.

mod common;

use common::{caps, is, list_all, mkdir, names, set_id, text_file, with_server};
use fs_interfaces::files_capnp::OnConflict;
use fs_interfaces::rpc::{read_envelope, write_uuid};

const NOT_FOUND: &str = "https://freesynergy.net/problems/files/not-found";
const CONFLICT: &str = "https://freesynergy.net/problems/files/conflict";

#[tokio::test(flavor = "current_thread")]
async fn folders_are_made_renamed_moved_and_the_disk_follows() {
    with_server(|root, fx| async move {
        let c = caps(&root).await;
        let trips = mkdir(&c, None, "Trips").await.unwrap();
        let rome = mkdir(&c, Some(trips), "Rome").await.unwrap();
        text_file(&c, Some(rome), "plan.txt", "colosseum").await;
        assert!(fx.root.join("Trips/Rome/plan.txt").is_file());

        let mut rename = c.folders.rename_folder_request();
        set_id(rename.get().init_id(), rome);
        rename.get().set_name("Roma");
        rename.get().set_on_conflict(OnConflict::Refuse);
        let answer = rename.send().promise.await.unwrap();
        let folder = answer.get().unwrap().get_folder().unwrap();
        let env = read_envelope(folder.get_envelope().unwrap()).unwrap();
        assert_eq!(
            (env.name.as_str(), env.object_type.as_str()),
            ("Roma", "Collection")
        );
        assert_eq!(env.context.as_deref(), Some("files"));
        assert!(fx.root.join("Trips/Roma/plan.txt").is_file());
        assert!(!fx.root.join("Trips/Rome").exists());

        let mut moved = c.folders.move_folder_request();
        set_id(moved.get().init_id(), rome);
        moved.get().init_parent();
        moved.get().set_on_conflict(OnConflict::Refuse);
        let answer = moved.send().promise.await.unwrap();
        let folder = answer.get().unwrap().get_folder().unwrap();
        assert!(fx.root.join("Roma/plan.txt").is_file());
        assert_eq!(common::read_uuid_of(folder.get_parent().unwrap()), None);

        let top = list_all(&c, None).await;
        assert_eq!(names(&top), ["Roma", "Trips"]);
        assert!(top.iter().all(|e| e.1), "both are folders");
    })
    .await;
}

#[tokio::test(flavor = "current_thread")]
async fn a_folder_is_found_by_id_and_a_stranger_is_not() {
    with_server(|root, _| async move {
        let c = caps(&root).await;
        let id = mkdir(&c, None, "Docs").await.unwrap();
        let mut get = c.folders.get_folder_request();
        set_id(get.get().init_id(), id);
        let answer = get.send().promise.await.unwrap();
        let got = answer.get().unwrap();
        assert!(got.get_found());
        let env = read_envelope(got.get_folder().unwrap().get_envelope().unwrap()).unwrap();
        assert_eq!((env.id.as_uuid(), env.name.as_str()), (id, "Docs"));

        let mut missing = c.folders.get_folder_request();
        set_id(missing.get().init_id(), common::unknown_id());
        let answer = missing.send().promise.await.unwrap();
        assert!(!answer.get().unwrap().get_found());
    })
    .await;
}

#[tokio::test(flavor = "current_thread")]
async fn delete_needs_an_empty_folder_and_a_folder_cannot_go_into_itself() {
    with_server(|root, fx| async move {
        let c = caps(&root).await;
        let outer = mkdir(&c, None, "Outer").await.unwrap();
        let inner = mkdir(&c, Some(outer), "Inner").await.unwrap();

        let mut delete = c.folders.delete_folder_request();
        set_id(delete.get().init_id(), outer);
        is(&delete.send().promise.await.err().unwrap(), CONFLICT, 409);

        let mut cycle = c.folders.move_folder_request();
        set_id(cycle.get().init_id(), outer);
        set_id(cycle.get().init_parent(), inner);
        cycle.get().set_on_conflict(OnConflict::Refuse);
        is(&cycle.send().promise.await.err().unwrap(), CONFLICT, 409);

        for id in [inner, outer] {
            let mut delete = c.folders.delete_folder_request();
            set_id(delete.get().init_id(), id);
            delete.send().promise.await.unwrap();
        }
        assert!(list_all(&c, None).await.is_empty());
        assert!(!fx.root.join("Outer").exists());

        let mut again = c.folders.delete_folder_request();
        set_id(again.get().init_id(), outer);
        is(&again.send().promise.await.err().unwrap(), NOT_FOUND, 404);
        let mut nowhere = c.folders.create_folder_request();
        write_uuid(nowhere.get().init_parent(), Some(common::unknown_id()));
        nowhere.get().set_name("x");
        is(&nowhere.send().promise.await.err().unwrap(), NOT_FOUND, 404);
    })
    .await;
}
