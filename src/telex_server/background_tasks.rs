use super::*;

// TODO: refactor
pub fn start_background_tasks() -> (Vec<VoidJoinHandle>, Vec<oneshot::Sender<()>>) {
    info!("spawning background tasks");

    let mut join_handles = Vec::new();

    let mut abort_senders = Vec::new();

    let (server_join_handles, mut server_senders, server_abort_senders) =
        update_other_servers(config!(SERVERS).to_vec());

    abort_senders.extend(server_abort_senders);

    join_handles.extend(server_join_handles);

    let name = "sync db";
    let (abort_sender, abort_receiver) = oneshot::channel();
    abort_senders.push(abort_sender);
    join_handles.push(task::spawn(async move {
        task::sleep(Duration::from_secs(1)).await;
        info!("starting {:?} background task", name);
        let mut exit = abort_receiver.fuse();
        loop {
            debug!("running background task {:?}", name);
            if let Err(err) = sync_db_to_disk().await {
                error!(
                    "{:?}",
                    anyhow!(err).context(format!("failed to run background task {}", name))
                );
            }
            select! {
                _ = exit => break,
                _ = task::sleep(config!(DB_SYNC_INTERVAL)).fuse() => continue,
            }
        }
        info!("stopped {:?} background task", name);
    }));

    let name = "sync changed";
    let (abort_sender, abort_receiver) = oneshot::channel();
    abort_senders.push(abort_sender);
    join_handles.push(task::spawn(async move {
        task::sleep(Duration::from_secs(3)).await;
        info!("starting {:?} background task", name);
        let mut exit = abort_receiver.fuse();
        loop {
            debug!("running background task {:?}", name);
            if let Err(err) = sync_changed(&mut server_senders).await {
                error!(
                    "{:?}",
                    anyhow!(err).context(format!("failed to run background task {}", name))
                );
            }
            select! {
                _ = exit => break,
                _ = task::sleep(config!(CHANGED_SYNC_INTERVAL)).fuse() => continue,
            }
        }
        info!("stopped {:?} background task", name);
    }));

    let name = "full query";
    let (abort_sender, abort_receiver) = oneshot::channel();
    abort_senders.push(abort_sender);
    join_handles.push(task::spawn(async move {
        task::sleep(Duration::from_secs(2)).await;
        info!("starting {:?} background task", name);
        let mut exit = abort_receiver.fuse();
        loop {
            debug!("running background task {:?}", name);
            if let Err(err) = full_query().await {
                error!(
                    "{:?}",
                    anyhow!(err).context(format!("failed to run background task {}", name))
                );
            }
            select! {
                _ = exit => break,
                _ = task::sleep(config!(FULL_QUERY_INTERVAL)).fuse() => continue,
            }
        }
        info!("stopped {:?} background task", name);
    }));

    info!("spawned background tasks");

    (join_handles, abort_senders)
}

async fn full_query_for_server(server: SocketAddr) -> anyhow::Result<()> {
    debug!("starting full query for server {}", server);

    let mut client = connect_to(server).await?;

    client.state = State::Accepting;

    let pkg = if config!(SERVER_PIN) == 0 {
        warn!(
            "Sending empty peer search instead of full query, because no server pin was specified"
        );

        Package::Type10(Package10 { version: PEER_SEARCH_VERSION, pattern: String::from("") })
    } else {
        Package::Type6(Package6 { version: FULL_QUERY_VERSION, server_pin: config!(SERVER_PIN) })
    };

    client.send_package(pkg).await?;

    wait_for_task(start_handling_client(client).await).await?;

    warn!("finished full query for server {}", server);

    Ok(())
}

async fn full_query() -> anyhow::Result<()> {
    let mut full_queries = Vec::new();

    info!("starting full query");

    for server in config!(SERVERS).iter() {
        full_queries.push(full_query_for_server(*server));
    }

    for result in futures::future::join_all(full_queries).await {
        if let Err(err) = result {
            error!("{:?}", anyhow!(err).context("A full query failed"));
        }
    }

    info!("finished full query");

    let n_changed = CHANGED.len();

    if n_changed > 0 {
        warn!("Server has {} changed entries", n_changed);
    }

    sync_db_to_disk().await?;

    Ok(()) //TODO
}

async fn connect_to(addr: SocketAddr) -> anyhow::Result<Client> {
    info!("connecting to server at {}", addr);

    Ok(Client::new(TcpStream::connect(addr).await?, addr))
}

async fn update_server_with_packages(server: SocketAddr, packages: Entries) -> anyhow::Result<()> {
    if config!(SERVER_PIN) == 0 {
        bail!(anyhow!("Not updating other servers without a server pin"));
    }

    let mut client = connect_to(server).await?;

    client.send_queue.extend(packages.into_iter());

    client.state = State::Responding;

    client
        .send_package(Package::Type7(Package7 {
            server_pin: config!(SERVER_PIN),
            version: LOGIN_VERSION,
        }))
        .await?;

    let task_id = start_handling_client(client).await;

    wait_for_task(task_id).await
}

fn update_other_servers(
    servers: Vec<SocketAddr>,
) -> (Vec<VoidJoinHandle>, Vec<mpsc::UnboundedSender<Entries>>, Vec<oneshot::Sender<()>>) {
    let mut join_handles = Vec::new();

    let mut senders = Vec::new();

    let mut abort_senders = Vec::new();

    for server in servers {
        let (abort_sender, abort_receiver) = oneshot::channel::<()>();

        abort_senders.push(abort_sender);

        let (sender, mut receiver) = mpsc::unbounded::<Entries>();

        senders.push(sender);

        join_handles.push(task::spawn(async move {
            info!("started syncing server: {}", server);

            let mut abort_receiver = abort_receiver.fuse();

            // NOTE: receiver already implementes `FusedStream` and so does not need to be
            // `fuse`ed
            'outer: while let Some(mut packages) = receiver.next().await {
                debug!("Received {} initial packages", packages.len());

                task::sleep(Duration::from_millis(10)).await;

                // Wait a bit, in case there are more packages on the way, but not yet in the
                // channel TODO: should we really do this?
                while let Ok(additional) = receiver.try_next() {
                    if let Some(additional) = additional {
                        debug!(
                            "Extending queue for client by {} additional packages",
                            additional.len()
                        );

                        packages.extend(additional);
                    } else {
                        break;
                    }
                }

                // This should never happen and be handled be handled by senders,
                // so that no task sync needs to take place:
                // TODO: remove?
                if packages.is_empty() {
                    debug!("There are no packages to sync");

                    continue;
                }

                while let Err(err) = update_server_with_packages(server, packages.clone()).await {
                    error!(
                        "{:?}",
                        anyhow!(err).context(format!("Failed to update server {}", server))
                    );

                    info!("retrying in: {:?}", config!(SERVER_COOLDOWN));

                    select! {
                        res = abort_receiver => if res.is_ok() { break 'outer; },
                        _ = task::sleep(config!(SERVER_COOLDOWN)).fuse() => {},
                    }
                }
            }

            info!("stopped syncing server: {}", server);
        }));
    }

    (join_handles, senders, abort_senders)
}

async fn sync_changed(
    server_senders: &mut Vec<mpsc::UnboundedSender<Entries>>,
) -> anyhow::Result<()> {
    let changed = get_changed_entries();

    if changed.is_empty() {
        return Ok(());
    }

    for sender in server_senders {
        sender.send(changed.clone()).await?;
    }

    Ok(())
}
