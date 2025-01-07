use meili_snap::snapshot;
use meilisearch_auth::AuthFilter;
use meilisearch_types::index_uid_pattern::IndexUidPattern;
use meilisearch_types::tasks::{IndexSwap, KindWithContent, Status};
use time::{Duration, OffsetDateTime};

use crate::insta_snapshot::{snapshot_bitmap, snapshot_index_scheduler};
use crate::test_utils::Breakpoint::*;
use crate::test_utils::{index_creation_task, FailureLocation};
use crate::{IndexScheduler, Query};

#[test]
fn query_tasks_from_and_limit() {
    let (index_scheduler, mut handle) = IndexScheduler::test(true, vec![]);

    let kind = index_creation_task("doggo", "bone");
    let _task = index_scheduler.register(kind, None, false).unwrap();
    snapshot!(snapshot_index_scheduler(&index_scheduler), name: "registered_the_first_task");
    let kind = index_creation_task("whalo", "plankton");
    let _task = index_scheduler.register(kind, None, false).unwrap();
    snapshot!(snapshot_index_scheduler(&index_scheduler), name: "registered_the_second_task");
    let kind = index_creation_task("catto", "his_own_vomit");
    let _task = index_scheduler.register(kind, None, false).unwrap();
    snapshot!(snapshot_index_scheduler(&index_scheduler), name: "registered_the_third_task");

    handle.advance_n_successful_batches(3);
    snapshot!(snapshot_index_scheduler(&index_scheduler), name: "processed_all_tasks");

    let rtxn = index_scheduler.env.read_txn().unwrap();
    let processing = index_scheduler.processing_tasks.read().unwrap();
    let query = Query { limit: Some(0), ..Default::default() };
    let (tasks, _) = index_scheduler
        .queue
        .get_task_ids_from_authorized_indexes(&rtxn, &query, &AuthFilter::default(), &processing)
        .unwrap();
    snapshot!(snapshot_bitmap(&tasks), @"[]");

    let query = Query { limit: Some(1), ..Default::default() };
    let (tasks, _) = index_scheduler
        .queue
        .get_task_ids_from_authorized_indexes(&rtxn, &query, &AuthFilter::default(), &processing)
        .unwrap();
    snapshot!(snapshot_bitmap(&tasks), @"[2,]");

    let query = Query { limit: Some(2), ..Default::default() };
    let (tasks, _) = index_scheduler
        .queue
        .get_task_ids_from_authorized_indexes(&rtxn, &query, &AuthFilter::default(), &processing)
        .unwrap();
    snapshot!(snapshot_bitmap(&tasks), @"[1,2,]");

    let query = Query { from: Some(1), ..Default::default() };
    let (tasks, _) = index_scheduler
        .queue
        .get_task_ids_from_authorized_indexes(&rtxn, &query, &AuthFilter::default(), &processing)
        .unwrap();
    snapshot!(snapshot_bitmap(&tasks), @"[0,1,]");

    let query = Query { from: Some(2), ..Default::default() };
    let (tasks, _) = index_scheduler
        .queue
        .get_task_ids_from_authorized_indexes(&rtxn, &query, &AuthFilter::default(), &processing)
        .unwrap();
    snapshot!(snapshot_bitmap(&tasks), @"[0,1,2,]");

    let query = Query { from: Some(1), limit: Some(1), ..Default::default() };
    let (tasks, _) = index_scheduler
        .queue
        .get_task_ids_from_authorized_indexes(&rtxn, &query, &AuthFilter::default(), &processing)
        .unwrap();
    snapshot!(snapshot_bitmap(&tasks), @"[1,]");

    let query = Query { from: Some(1), limit: Some(2), ..Default::default() };
    let (tasks, _) = index_scheduler
        .queue
        .get_task_ids_from_authorized_indexes(&rtxn, &query, &AuthFilter::default(), &processing)
        .unwrap();
    snapshot!(snapshot_bitmap(&tasks), @"[0,1,]");
}

#[test]
fn query_tasks_simple() {
    let start_time = OffsetDateTime::now_utc();

    let (index_scheduler, mut handle) =
        IndexScheduler::test(true, vec![(3, FailureLocation::InsideProcessBatch)]);

    let kind = index_creation_task("catto", "mouse");
    let _task = index_scheduler.register(kind, None, false).unwrap();
    let kind = index_creation_task("doggo", "sheep");
    let _task = index_scheduler.register(kind, None, false).unwrap();
    let kind = index_creation_task("whalo", "fish");
    let _task = index_scheduler.register(kind, None, false).unwrap();

    snapshot!(snapshot_index_scheduler(&index_scheduler), name: "start");

    handle.advance_till([Start, BatchCreated]);

    let query = Query { statuses: Some(vec![Status::Processing]), ..Default::default() };
    let (tasks, _) = index_scheduler
        .get_task_ids_from_authorized_indexes(&query, &AuthFilter::default())
        .unwrap();
    snapshot!(snapshot_bitmap(&tasks), @"[0,]"); // only the processing tasks in the first tick

    let query = Query { statuses: Some(vec![Status::Enqueued]), ..Default::default() };
    let (tasks, _) = index_scheduler
        .get_task_ids_from_authorized_indexes(&query, &AuthFilter::default())
        .unwrap();
    snapshot!(snapshot_bitmap(&tasks), @"[1,2,]"); // only the enqueued tasks in the first tick

    let query =
        Query { statuses: Some(vec![Status::Enqueued, Status::Processing]), ..Default::default() };
    let (tasks, _) = index_scheduler
        .get_task_ids_from_authorized_indexes(&query, &AuthFilter::default())
        .unwrap();
    snapshot!(snapshot_bitmap(&tasks), @"[0,1,2,]"); // both enqueued and processing tasks in the first tick

    let query = Query {
        statuses: Some(vec![Status::Enqueued, Status::Processing]),
        after_started_at: Some(start_time),
        ..Default::default()
    };
    let (tasks, _) = index_scheduler
        .get_task_ids_from_authorized_indexes(&query, &AuthFilter::default())
        .unwrap();
    // both enqueued and processing tasks in the first tick, but limited to those with a started_at
    // that comes after the start of the test, which should excludes the enqueued tasks
    snapshot!(snapshot_bitmap(&tasks), @"[0,]");

    let query = Query {
        statuses: Some(vec![Status::Enqueued, Status::Processing]),
        before_started_at: Some(start_time),
        ..Default::default()
    };
    let (tasks, _) = index_scheduler
        .get_task_ids_from_authorized_indexes(&query, &AuthFilter::default())
        .unwrap();
    // both enqueued and processing tasks in the first tick, but limited to those with a started_at
    // that comes before the start of the test, which should excludes all of them
    snapshot!(snapshot_bitmap(&tasks), @"[]");

    let query = Query {
        statuses: Some(vec![Status::Enqueued, Status::Processing]),
        after_started_at: Some(start_time),
        before_started_at: Some(start_time + Duration::minutes(1)),
        ..Default::default()
    };
    let (tasks, _) = index_scheduler
        .get_task_ids_from_authorized_indexes(&query, &AuthFilter::default())
        .unwrap();
    // both enqueued and processing tasks in the first tick, but limited to those with a started_at
    // that comes after the start of the test and before one minute after the start of the test,
    // which should exclude the enqueued tasks and include the only processing task
    snapshot!(snapshot_bitmap(&tasks), @"[0,]");

    handle.advance_till([
        InsideProcessBatch,
        InsideProcessBatch,
        ProcessBatchSucceeded,
        AfterProcessing,
        Start,
        BatchCreated,
    ]);

    let second_start_time = OffsetDateTime::now_utc();

    let query = Query {
        statuses: Some(vec![Status::Succeeded, Status::Processing]),
        after_started_at: Some(start_time),
        before_started_at: Some(start_time + Duration::minutes(1)),
        ..Default::default()
    };
    let (tasks, _) = index_scheduler
        .get_task_ids_from_authorized_indexes(&query, &AuthFilter::default())
        .unwrap();
    // both succeeded and processing tasks in the first tick, but limited to those with a started_at
    // that comes after the start of the test and before one minute after the start of the test,
    // which should include all tasks
    snapshot!(snapshot_bitmap(&tasks), @"[0,1,]");

    let query = Query {
        statuses: Some(vec![Status::Succeeded, Status::Processing]),
        before_started_at: Some(start_time),
        ..Default::default()
    };
    let (tasks, _) = index_scheduler
        .get_task_ids_from_authorized_indexes(&query, &AuthFilter::default())
        .unwrap();
    // both succeeded and processing tasks in the first tick, but limited to those with a started_at
    // that comes before the start of the test, which should exclude all tasks
    snapshot!(snapshot_bitmap(&tasks), @"[]");

    let query = Query {
        statuses: Some(vec![Status::Enqueued, Status::Succeeded, Status::Processing]),
        after_started_at: Some(second_start_time),
        before_started_at: Some(second_start_time + Duration::minutes(1)),
        ..Default::default()
    };
    let (tasks, _) = index_scheduler
        .get_task_ids_from_authorized_indexes(&query, &AuthFilter::default())
        .unwrap();
    // both succeeded and processing tasks in the first tick, but limited to those with a started_at
    // that comes after the start of the second part of the test and before one minute after the
    // second start of the test, which should exclude all tasks
    snapshot!(snapshot_bitmap(&tasks), @"[]");

    // now we make one more batch, the started_at field of the new tasks will be past `second_start_time`
    handle.advance_till([
        InsideProcessBatch,
        InsideProcessBatch,
        ProcessBatchSucceeded,
        AfterProcessing,
        Start,
        BatchCreated,
    ]);

    let (tasks, _) = index_scheduler
        .get_task_ids_from_authorized_indexes(&query, &AuthFilter::default())
        .unwrap();
    // we run the same query to verify that, and indeed find that the last task is matched
    snapshot!(snapshot_bitmap(&tasks), @"[2,]");

    let query = Query {
        statuses: Some(vec![Status::Enqueued, Status::Succeeded, Status::Processing]),
        after_started_at: Some(second_start_time),
        before_started_at: Some(second_start_time + Duration::minutes(1)),
        ..Default::default()
    };
    let (tasks, _) = index_scheduler
        .get_task_ids_from_authorized_indexes(&query, &AuthFilter::default())
        .unwrap();
    // enqueued, succeeded, or processing tasks started after the second part of the test, should
    // again only return the last task
    snapshot!(snapshot_bitmap(&tasks), @"[2,]");

    handle.advance_till([ProcessBatchFailed, AfterProcessing]);

    // now the last task should have failed
    snapshot!(snapshot_index_scheduler(&index_scheduler), name: "end");
    let (tasks, _) = index_scheduler
        .get_task_ids_from_authorized_indexes(&query, &AuthFilter::default())
        .unwrap();
    // so running the last query should return nothing
    snapshot!(snapshot_bitmap(&tasks), @"[]");

    let query = Query {
        statuses: Some(vec![Status::Failed]),
        after_started_at: Some(second_start_time),
        before_started_at: Some(second_start_time + Duration::minutes(1)),
        ..Default::default()
    };
    let (tasks, _) = index_scheduler
        .get_task_ids_from_authorized_indexes(&query, &AuthFilter::default())
        .unwrap();
    // but the same query on failed tasks should return the last task
    snapshot!(snapshot_bitmap(&tasks), @"[2,]");

    let query = Query {
        statuses: Some(vec![Status::Failed]),
        after_started_at: Some(second_start_time),
        before_started_at: Some(second_start_time + Duration::minutes(1)),
        ..Default::default()
    };
    let (tasks, _) = index_scheduler
        .get_task_ids_from_authorized_indexes(&query, &AuthFilter::default())
        .unwrap();
    // but the same query on failed tasks should return the last task
    snapshot!(snapshot_bitmap(&tasks), @"[2,]");

    let query = Query {
        statuses: Some(vec![Status::Failed]),
        uids: Some(vec![1]),
        after_started_at: Some(second_start_time),
        before_started_at: Some(second_start_time + Duration::minutes(1)),
        ..Default::default()
    };
    let (tasks, _) = index_scheduler
        .get_task_ids_from_authorized_indexes(&query, &AuthFilter::default())
        .unwrap();
    // same query but with an invalid uid
    snapshot!(snapshot_bitmap(&tasks), @"[]");

    let query = Query {
        statuses: Some(vec![Status::Failed]),
        uids: Some(vec![2]),
        after_started_at: Some(second_start_time),
        before_started_at: Some(second_start_time + Duration::minutes(1)),
        ..Default::default()
    };
    let (tasks, _) = index_scheduler
        .get_task_ids_from_authorized_indexes(&query, &AuthFilter::default())
        .unwrap();
    // same query but with a valid uid
    snapshot!(snapshot_bitmap(&tasks), @"[2,]");
}

#[test]
fn query_tasks_special_rules() {
    let (index_scheduler, mut handle) =
        IndexScheduler::test(true, vec![(3, FailureLocation::InsideProcessBatch)]);

    let kind = index_creation_task("catto", "mouse");
    let _task = index_scheduler.register(kind, None, false).unwrap();
    let kind = index_creation_task("doggo", "sheep");
    let _task = index_scheduler.register(kind, None, false).unwrap();
    let kind = KindWithContent::IndexSwap {
        swaps: vec![IndexSwap { indexes: ("catto".to_owned(), "doggo".to_owned()) }],
    };
    let _task = index_scheduler.register(kind, None, false).unwrap();
    let kind = KindWithContent::IndexSwap {
        swaps: vec![IndexSwap { indexes: ("catto".to_owned(), "whalo".to_owned()) }],
    };
    let _task = index_scheduler.register(kind, None, false).unwrap();

    snapshot!(snapshot_index_scheduler(&index_scheduler), name: "start");

    handle.advance_till([Start, BatchCreated]);

    let rtxn = index_scheduler.env.read_txn().unwrap();
    let proc = index_scheduler.processing_tasks.read().unwrap();

    let query = Query { index_uids: Some(vec!["catto".to_owned()]), ..Default::default() };
    let (tasks, _) = index_scheduler
        .queue
        .get_task_ids_from_authorized_indexes(&rtxn, &query, &AuthFilter::default(), &proc)
        .unwrap();
    // only the first task associated with catto is returned, the indexSwap tasks are excluded!
    snapshot!(snapshot_bitmap(&tasks), @"[0,]");

    let query = Query { index_uids: Some(vec!["catto".to_owned()]), ..Default::default() };
    let (tasks, _) = index_scheduler
        .queue
        .get_task_ids_from_authorized_indexes(
            &rtxn,
            &query,
            &AuthFilter::with_allowed_indexes(
                vec![IndexUidPattern::new_unchecked("doggo")].into_iter().collect(),
            ),
            &proc,
        )
        .unwrap();
    // we have asked for only the tasks associated with catto, but are only authorized to retrieve the tasks
    // associated with doggo -> empty result
    snapshot!(snapshot_bitmap(&tasks), @"[]");

    let query = Query::default();
    let (tasks, _) = index_scheduler
        .queue
        .get_task_ids_from_authorized_indexes(
            &rtxn,
            &query,
            &AuthFilter::with_allowed_indexes(
                vec![IndexUidPattern::new_unchecked("doggo")].into_iter().collect(),
            ),
            &proc,
        )
        .unwrap();
    // we asked for all the tasks, but we are only authorized to retrieve the doggo tasks
    // -> only the index creation of doggo should be returned
    snapshot!(snapshot_bitmap(&tasks), @"[1,]");

    let query = Query::default();
    let (tasks, _) = index_scheduler
        .queue
        .get_task_ids_from_authorized_indexes(
            &rtxn,
            &query,
            &AuthFilter::with_allowed_indexes(
                vec![
                    IndexUidPattern::new_unchecked("catto"),
                    IndexUidPattern::new_unchecked("doggo"),
                ]
                .into_iter()
                .collect(),
            ),
            &proc,
        )
        .unwrap();
    // we asked for all the tasks, but we are only authorized to retrieve the doggo and catto tasks
    // -> all tasks except the swap of catto with whalo are returned
    snapshot!(snapshot_bitmap(&tasks), @"[0,1,]");

    let query = Query::default();
    let (tasks, _) = index_scheduler
        .queue
        .get_task_ids_from_authorized_indexes(&rtxn, &query, &AuthFilter::default(), &proc)
        .unwrap();
    // we asked for all the tasks with all index authorized -> all tasks returned
    snapshot!(snapshot_bitmap(&tasks), @"[0,1,2,3,]");
}

#[test]
fn query_tasks_canceled_by() {
    let (index_scheduler, mut handle) =
        IndexScheduler::test(true, vec![(3, FailureLocation::InsideProcessBatch)]);

    let kind = index_creation_task("catto", "mouse");
    let _ = index_scheduler.register(kind, None, false).unwrap();
    let kind = index_creation_task("doggo", "sheep");
    let _ = index_scheduler.register(kind, None, false).unwrap();
    let kind = KindWithContent::IndexSwap {
        swaps: vec![IndexSwap { indexes: ("catto".to_owned(), "doggo".to_owned()) }],
    };
    let _task = index_scheduler.register(kind, None, false).unwrap();

    handle.advance_n_successful_batches(1);
    let kind = KindWithContent::TaskCancelation {
        query: "test_query".to_string(),
        tasks: [0, 1, 2, 3].into_iter().collect(),
    };
    let task_cancelation = index_scheduler.register(kind, None, false).unwrap();
    handle.advance_n_successful_batches(1);

    snapshot!(snapshot_index_scheduler(&index_scheduler), name: "start");

    let rtxn = index_scheduler.read_txn().unwrap();
    let proc = index_scheduler.processing_tasks.read().unwrap();
    let query = Query { canceled_by: Some(vec![task_cancelation.uid]), ..Query::default() };
    let (tasks, _) = index_scheduler
        .queue
        .get_task_ids_from_authorized_indexes(&rtxn, &query, &AuthFilter::default(), &proc)
        .unwrap();
    // 0 is not returned because it was not canceled, 3 is not returned because it is the uid of the
    // taskCancelation itself
    snapshot!(snapshot_bitmap(&tasks), @"[1,2,]");

    let query = Query { canceled_by: Some(vec![task_cancelation.uid]), ..Query::default() };
    let (tasks, _) = index_scheduler
        .queue
        .get_task_ids_from_authorized_indexes(
            &rtxn,
            &query,
            &AuthFilter::with_allowed_indexes(
                vec![IndexUidPattern::new_unchecked("doggo")].into_iter().collect(),
            ),
            &proc,
        )
        .unwrap();
    // Return only 1 because the user is not authorized to see task 2
    snapshot!(snapshot_bitmap(&tasks), @"[1,]");
}
