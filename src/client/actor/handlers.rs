use crate::{
    client::{file_operations::Client, utils, CHUNK_SIZE, Blacklist},
    messaging::{
        self,
        arguments::{FileId, FileList, FileMetadata, FilePath, QualifiedChunk, QualifiedChunkId},
        Message,
    },
};
use log::{debug, error, info};
use notify::DebouncedEvent;
use std::{collections::HashSet, fs::File, path::Path};

pub async fn handle_server_event(
    client: &mut Client,
    watch_path: &Path,
    event: Message,
    blacklist: &mut Blacklist,
) {
    let verb = event.verb.clone();
    match verb {
        messaging::Directive::SendFiles => {
            let files = utils::generate_file_list(watch_path).unwrap();
            let mut local_files: HashSet<FileId> = HashSet::new();
            for file in files.0 {
                local_files.insert(file);
            }

            let mut server_files: HashSet<FileId> = HashSet::new();

            if let Some(argument) = event.argument {
                let files = argument.as_any().downcast_ref::<FileList>().unwrap();

                for file in &files.0 {
                    server_files.insert(file.clone());
                }
            }

            for file in local_files.difference(&server_files) {
                debug!("File not found on server: {:?}", file.path);
                client
                    .send_file_info(watch_path, &watch_path.join(&file.path))
                    .await
                    .unwrap();
            }
            for file in server_files.difference(&local_files) {
                debug!("File not found locally: {:?}", file.path);
                let _ = client.request_file(file.clone()).await;
            }
        }
        messaging::Directive::RequestFile => todo!(),
        messaging::Directive::RequestChunk => {
            if let Some(argument) = event.argument {
                let chunk: &QualifiedChunkId = argument
                    .as_any()
                    .downcast_ref::<QualifiedChunkId>()
                    .unwrap();
                let path = watch_path.join(chunk.path.path.clone());
                client
                    .send_chunk(&chunk.id, &path)
                    .await
                    .expect("Failed to queue chunk");
            }
        }
        messaging::Directive::SendFile => {
            if let Some(argument) = event.argument {
                let file_md = argument.as_any().downcast_ref::<FileMetadata>().unwrap();
                let path = file_md.file_id.path.clone();
                // The blacklist needs to be updated to make sure we dont send file information for
                // a in progress transfer
                debug!("adding to blacklist");
                blacklist.insert(path, file_md.clone());
                let mut _file = File::create(watch_path.join(&file_md.file_id.path)).unwrap();
                info!("Started file download: {:?}", &file_md.file_id.path);
                for (i, chunk) in file_md.chunks.iter().enumerate() {
                    let q_chunk = QualifiedChunkId {
                        path: file_md.file_id.clone(),
                        offset: (i * CHUNK_SIZE) as u32,
                        id: chunk.clone(),
                    };
                    client.request_chunk(q_chunk).await.unwrap();
                }
            }
        }
        messaging::Directive::SendQualifiedChunk => {
            if let Some(argument) = event.argument {
                if let Err(e) = utils::write_chunk(
                    blacklist,
                    &watch_path.canonicalize().unwrap(),
                    argument.as_any().downcast_ref::<QualifiedChunk>().unwrap(),
                ) {
                    error!("{}", e);
                }
            }
        }
        messaging::Directive::DeleteFile => {
            if let Some(argument) = event.argument {
                let fpath = argument.as_any().downcast_ref::<FilePath>().unwrap();
                debug!("Got file deletion of {:?}", fpath);
                let _ = tokio::fs::remove_file(watch_path.join(&fpath.0)).await;
            }
        }
        _ => {}
    };
}

pub async fn handle_fs_event(
    client: &mut Client,
    watch_path: &Path,
    event: DebouncedEvent,
    blacklist: &mut Blacklist,
) {
    match event {
        DebouncedEvent::Rename(_, p)
        | DebouncedEvent::Create(p)
        | DebouncedEvent::Write(p)
        | DebouncedEvent::Chmod(p) => {
            // Check the blacklist to make sure the event isn't from a partial file transfer
            if !blacklist.contains_key(p.strip_prefix(watch_path).unwrap()) {
                match client.send_file_info(watch_path, &p).await {
                    Ok(_) => {
                        info!("Successfully sent the file");
                    }
                    Err(e) => error!("{:?}", e),
                };
            }
        }
        DebouncedEvent::Remove(p) => {
            match client
                .delete_file(FilePath::new(p.strip_prefix(watch_path).unwrap()))
                .await
            {
                Ok(_) => info!("Successfully deleted the file"),
                Err(e) => error!("{:?}", e),
            }
        }
        _ => {}
    }
}
