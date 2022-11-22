(function() {var implementors = {
"concurrent_ringbuf":[["impl&lt;T&gt; <a class=\"trait\" href=\"https://doc.rust-lang.org/1.65.0/core/marker/trait.Send.html\" title=\"trait core::marker::Send\">Send</a> for <a class=\"enum\" href=\"concurrent_ringbuf/enum.Steal.html\" title=\"enum concurrent_ringbuf::Steal\">Steal</a>&lt;T&gt;<span class=\"where fmt-newline\">where<br>&nbsp;&nbsp;&nbsp;&nbsp;T: <a class=\"trait\" href=\"https://doc.rust-lang.org/1.65.0/core/marker/trait.Send.html\" title=\"trait core::marker::Send\">Send</a>,</span>",1,["concurrent_ringbuf::Steal"]],["impl&lt;T&gt; <a class=\"trait\" href=\"https://doc.rust-lang.org/1.65.0/core/marker/trait.Send.html\" title=\"trait core::marker::Send\">Send</a> for <a class=\"struct\" href=\"concurrent_ringbuf/struct.Ringbuf.html\" title=\"struct concurrent_ringbuf::Ringbuf\">Ringbuf</a>&lt;T&gt;",1,["concurrent_ringbuf::Ringbuf"]],["impl&lt;T&gt; <a class=\"trait\" href=\"https://doc.rust-lang.org/1.65.0/core/marker/trait.Send.html\" title=\"trait core::marker::Send\">Send</a> for <a class=\"struct\" href=\"concurrent_ringbuf/struct.Stealer.html\" title=\"struct concurrent_ringbuf::Stealer\">Stealer</a>&lt;T&gt;",1,["concurrent_ringbuf::Stealer"]]],
"my_async":[["impl&lt;'a&gt; <a class=\"trait\" href=\"https://doc.rust-lang.org/1.65.0/core/marker/trait.Send.html\" title=\"trait core::marker::Send\">Send</a> for <a class=\"struct\" href=\"my_async/net/struct.TcpIncoming.html\" title=\"struct my_async::net::TcpIncoming\">TcpIncoming</a>&lt;'a&gt;",1,["my_async::modules::net::TcpIncoming"]],["impl&lt;'a&gt; <a class=\"trait\" href=\"https://doc.rust-lang.org/1.65.0/core/marker/trait.Send.html\" title=\"trait core::marker::Send\">Send</a> for <a class=\"struct\" href=\"my_async/net/struct.UnixIncoming.html\" title=\"struct my_async::net::UnixIncoming\">UnixIncoming</a>&lt;'a&gt;",1,["my_async::modules::net::UnixIncoming"]],["impl <a class=\"trait\" href=\"https://doc.rust-lang.org/1.65.0/core/marker/trait.Send.html\" title=\"trait core::marker::Send\">Send</a> for <a class=\"struct\" href=\"my_async/multi_thread/struct.FutureIndex.html\" title=\"struct my_async::multi_thread::FutureIndex\">FutureIndex</a>",1,["my_async::multi_thread::FutureIndex"]],["impl&lt;S&gt; <a class=\"trait\" href=\"https://doc.rust-lang.org/1.65.0/core/marker/trait.Send.html\" title=\"trait core::marker::Send\">Send</a> for <a class=\"struct\" href=\"my_async/multi_thread/struct.Executor.html\" title=\"struct my_async::multi_thread::Executor\">Executor</a>&lt;S&gt;<span class=\"where fmt-newline\">where<br>&nbsp;&nbsp;&nbsp;&nbsp;S: <a class=\"trait\" href=\"https://doc.rust-lang.org/1.65.0/core/marker/trait.Send.html\" title=\"trait core::marker::Send\">Send</a>,</span>",1,["my_async::multi_thread::Executor"]],["impl <a class=\"trait\" href=\"https://doc.rust-lang.org/1.65.0/core/marker/trait.Send.html\" title=\"trait core::marker::Send\">Send</a> for <a class=\"struct\" href=\"my_async/single_thread/struct.Executor.html\" title=\"struct my_async::single_thread::Executor\">Executor</a>",1,["my_async::single_thread::Executor"]],["impl <a class=\"trait\" href=\"https://doc.rust-lang.org/1.65.0/core/marker/trait.Send.html\" title=\"trait core::marker::Send\">Send</a> for <a class=\"struct\" href=\"my_async/schedulers/hybrid/struct.HybridScheduler.html\" title=\"struct my_async::schedulers::hybrid::HybridScheduler\">HybridScheduler</a>",1,["my_async::schedulers::hybrid::HybridScheduler"]],["impl <a class=\"trait\" href=\"https://doc.rust-lang.org/1.65.0/core/marker/trait.Send.html\" title=\"trait core::marker::Send\">Send</a> for <a class=\"struct\" href=\"my_async/schedulers/round_robin/struct.RoundRobinScheduler.html\" title=\"struct my_async::schedulers::round_robin::RoundRobinScheduler\">RoundRobinScheduler</a>",1,["my_async::schedulers::round_robin::RoundRobinScheduler"]],["impl <a class=\"trait\" href=\"https://doc.rust-lang.org/1.65.0/core/marker/trait.Send.html\" title=\"trait core::marker::Send\">Send</a> for <a class=\"struct\" href=\"my_async/schedulers/work_stealing/struct.WorkStealingScheduler.html\" title=\"struct my_async::schedulers::work_stealing::WorkStealingScheduler\">WorkStealingScheduler</a>",1,["my_async::schedulers::work_stealing::WorkStealingScheduler"]],["impl <a class=\"trait\" href=\"https://doc.rust-lang.org/1.65.0/core/marker/trait.Send.html\" title=\"trait core::marker::Send\">Send</a> for <a class=\"enum\" href=\"my_async/schedulers/enum.ScheduleMessage.html\" title=\"enum my_async::schedulers::ScheduleMessage\">ScheduleMessage</a>",1,["my_async::schedulers::ScheduleMessage"]],["impl <a class=\"trait\" href=\"https://doc.rust-lang.org/1.65.0/core/marker/trait.Send.html\" title=\"trait core::marker::Send\">Send</a> for <a class=\"struct\" href=\"my_async/schedulers/struct.Spawner.html\" title=\"struct my_async::schedulers::Spawner\">Spawner</a>",1,["my_async::schedulers::Spawner"]],["impl&lt;T&gt; <a class=\"trait\" href=\"https://doc.rust-lang.org/1.65.0/core/marker/trait.Send.html\" title=\"trait core::marker::Send\">Send</a> for <a class=\"struct\" href=\"my_async/schedulers/struct.JoinHandle.html\" title=\"struct my_async::schedulers::JoinHandle\">JoinHandle</a>&lt;T&gt;<span class=\"where fmt-newline\">where<br>&nbsp;&nbsp;&nbsp;&nbsp;T: <a class=\"trait\" href=\"https://doc.rust-lang.org/1.65.0/core/marker/trait.Send.html\" title=\"trait core::marker::Send\">Send</a>,</span>",1,["my_async::schedulers::JoinHandle"]],["impl&lt;'a, T&gt; <a class=\"trait\" href=\"https://doc.rust-lang.org/1.65.0/core/marker/trait.Send.html\" title=\"trait core::marker::Send\">Send</a> for <a class=\"struct\" href=\"my_async/schedulers/struct.FutureJoin.html\" title=\"struct my_async::schedulers::FutureJoin\">FutureJoin</a>&lt;'a, T&gt;<span class=\"where fmt-newline\">where<br>&nbsp;&nbsp;&nbsp;&nbsp;T: <a class=\"trait\" href=\"https://doc.rust-lang.org/1.65.0/core/marker/trait.Send.html\" title=\"trait core::marker::Send\">Send</a>,</span>",1,["my_async::schedulers::FutureJoin"]],["impl&lt;T&gt; <a class=\"trait\" href=\"https://doc.rust-lang.org/1.65.0/core/marker/trait.Send.html\" title=\"trait core::marker::Send\">Send</a> for <a class=\"struct\" href=\"my_async/struct.IoWrapper.html\" title=\"struct my_async::IoWrapper\">IoWrapper</a>&lt;T&gt;<span class=\"where fmt-newline\">where<br>&nbsp;&nbsp;&nbsp;&nbsp;T: <a class=\"trait\" href=\"https://doc.rust-lang.org/1.65.0/core/marker/trait.Send.html\" title=\"trait core::marker::Send\">Send</a>,</span>",1,["my_async::IoWrapper"]]]
};if (window.register_implementors) {window.register_implementors(implementors);} else {window.pending_implementors = implementors;}})()