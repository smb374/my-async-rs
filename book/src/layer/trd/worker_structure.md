# General Worker structure and logic
Next we will define the worker's general structure and logic.
Since a worker's structure and behavior is implementation specific,
it's hard to use a trait and rule them all, but the general idea is shared: Receive and Execute.

The loop of processing can divide into 3 stages:
1. Process all the tasks in local queue.
2. Non-blocking collect any of the woke up tasks. If there are any, push them into local queue and continue.
3. Wait for woke up tasks and new tasks. Push these tasks into local queue and continue.

We can then turn it to a pseudo code:
```
fn process():
    while true:
        if Some(t) <- q.pop():
            t.run()
        else:
            cnt <- 0
            while Ok(t) <- wake_receiver.try_recv() or Ok(t) <- new_task_receiver.try_recv():
                q.push(t)
                cnt <- cnt + 1
            if cnt > 0:
                continue
            if t <- wake_receiver.recv() or t <- new_task_receiver.recv():
                q.push(t)
                continue
            else if shutdown <- notifier.recv():
                break
```
The code did the following stuff:
1. If local queue contains some tasks, execute them.
2. Try to receive any scheduled or woke up tasks. If there any, push them and continue the loop.
3. Block receive any task that is scheduled or woke up. If there is one, push it and continue the loop.
4. If received a shutdown signal, break the process loop.

The schedulers that I implemented myself follows this structure, with many specific stuff due to the scheduling method.

