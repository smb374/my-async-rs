(function() {var implementors = {};
implementors["concurrent_ringbuf"] = [{"text":"impl&lt;T&gt; <a class=\"trait\" href=\"https://doc.rust-lang.org/1.64.0/core/marker/trait.Unpin.html\" title=\"trait core::marker::Unpin\">Unpin</a> for <a class=\"enum\" href=\"concurrent_ringbuf/enum.Steal.html\" title=\"enum concurrent_ringbuf::Steal\">Steal</a>&lt;T&gt; <span class=\"where fmt-newline\">where<br>&nbsp;&nbsp;&nbsp;&nbsp;T: <a class=\"trait\" href=\"https://doc.rust-lang.org/1.64.0/core/marker/trait.Unpin.html\" title=\"trait core::marker::Unpin\">Unpin</a>,&nbsp;</span>","synthetic":true,"types":["concurrent_ringbuf::Steal"]},{"text":"impl&lt;T&gt; <a class=\"trait\" href=\"https://doc.rust-lang.org/1.64.0/core/marker/trait.Unpin.html\" title=\"trait core::marker::Unpin\">Unpin</a> for <a class=\"struct\" href=\"concurrent_ringbuf/struct.Ringbuf.html\" title=\"struct concurrent_ringbuf::Ringbuf\">Ringbuf</a>&lt;T&gt;","synthetic":true,"types":["concurrent_ringbuf::Ringbuf"]},{"text":"impl&lt;T&gt; <a class=\"trait\" href=\"https://doc.rust-lang.org/1.64.0/core/marker/trait.Unpin.html\" title=\"trait core::marker::Unpin\">Unpin</a> for <a class=\"struct\" href=\"concurrent_ringbuf/struct.Stealer.html\" title=\"struct concurrent_ringbuf::Stealer\">Stealer</a>&lt;T&gt;","synthetic":true,"types":["concurrent_ringbuf::Stealer"]}];
implementors["my_async"] = [{"text":"impl&lt;'a&gt; <a class=\"trait\" href=\"https://doc.rust-lang.org/1.64.0/core/marker/trait.Unpin.html\" title=\"trait core::marker::Unpin\">Unpin</a> for <a class=\"struct\" href=\"my_async/net/struct.TcpIncoming.html\" title=\"struct my_async::net::TcpIncoming\">TcpIncoming</a>&lt;'a&gt;","synthetic":true,"types":["my_async::modules::net::TcpIncoming"]},{"text":"impl&lt;'a&gt; <a class=\"trait\" href=\"https://doc.rust-lang.org/1.64.0/core/marker/trait.Unpin.html\" title=\"trait core::marker::Unpin\">Unpin</a> for <a class=\"struct\" href=\"my_async/net/struct.UnixIncoming.html\" title=\"struct my_async::net::UnixIncoming\">UnixIncoming</a>&lt;'a&gt;","synthetic":true,"types":["my_async::modules::net::UnixIncoming"]},{"text":"impl <a class=\"trait\" href=\"https://doc.rust-lang.org/1.64.0/core/marker/trait.Unpin.html\" title=\"trait core::marker::Unpin\">Unpin</a> for <a class=\"struct\" href=\"my_async/multi_thread/struct.FutureIndex.html\" title=\"struct my_async::multi_thread::FutureIndex\">FutureIndex</a>","synthetic":true,"types":["my_async::multi_thread::FutureIndex"]},{"text":"impl&lt;S&gt; <a class=\"trait\" href=\"https://doc.rust-lang.org/1.64.0/core/marker/trait.Unpin.html\" title=\"trait core::marker::Unpin\">Unpin</a> for <a class=\"struct\" href=\"my_async/multi_thread/struct.Executor.html\" title=\"struct my_async::multi_thread::Executor\">Executor</a>&lt;S&gt; <span class=\"where fmt-newline\">where<br>&nbsp;&nbsp;&nbsp;&nbsp;S: <a class=\"trait\" href=\"https://doc.rust-lang.org/1.64.0/core/marker/trait.Unpin.html\" title=\"trait core::marker::Unpin\">Unpin</a>,&nbsp;</span>","synthetic":true,"types":["my_async::multi_thread::Executor"]},{"text":"impl <a class=\"trait\" href=\"https://doc.rust-lang.org/1.64.0/core/marker/trait.Unpin.html\" title=\"trait core::marker::Unpin\">Unpin</a> for <a class=\"struct\" href=\"my_async/single_thread/struct.Executor.html\" title=\"struct my_async::single_thread::Executor\">Executor</a>","synthetic":true,"types":["my_async::single_thread::Executor"]},{"text":"impl <a class=\"trait\" href=\"https://doc.rust-lang.org/1.64.0/core/marker/trait.Unpin.html\" title=\"trait core::marker::Unpin\">Unpin</a> for <a class=\"struct\" href=\"my_async/schedulers/hybrid/struct.HybridScheduler.html\" title=\"struct my_async::schedulers::hybrid::HybridScheduler\">HybridScheduler</a>","synthetic":true,"types":["my_async::schedulers::hybrid::HybridScheduler"]},{"text":"impl <a class=\"trait\" href=\"https://doc.rust-lang.org/1.64.0/core/marker/trait.Unpin.html\" title=\"trait core::marker::Unpin\">Unpin</a> for <a class=\"struct\" href=\"my_async/schedulers/round_robin/struct.RoundRobinScheduler.html\" title=\"struct my_async::schedulers::round_robin::RoundRobinScheduler\">RoundRobinScheduler</a>","synthetic":true,"types":["my_async::schedulers::round_robin::RoundRobinScheduler"]},{"text":"impl <a class=\"trait\" href=\"https://doc.rust-lang.org/1.64.0/core/marker/trait.Unpin.html\" title=\"trait core::marker::Unpin\">Unpin</a> for <a class=\"struct\" href=\"my_async/schedulers/round_robin/struct.Worker.html\" title=\"struct my_async::schedulers::round_robin::Worker\">Worker</a>","synthetic":true,"types":["my_async::schedulers::round_robin::Worker"]},{"text":"impl <a class=\"trait\" href=\"https://doc.rust-lang.org/1.64.0/core/marker/trait.Unpin.html\" title=\"trait core::marker::Unpin\">Unpin</a> for <a class=\"struct\" href=\"my_async/schedulers/work_stealing/struct.WorkStealingScheduler.html\" title=\"struct my_async::schedulers::work_stealing::WorkStealingScheduler\">WorkStealingScheduler</a>","synthetic":true,"types":["my_async::schedulers::work_stealing::WorkStealingScheduler"]},{"text":"impl <a class=\"trait\" href=\"https://doc.rust-lang.org/1.64.0/core/marker/trait.Unpin.html\" title=\"trait core::marker::Unpin\">Unpin</a> for <a class=\"enum\" href=\"my_async/schedulers/enum.ScheduleMessage.html\" title=\"enum my_async::schedulers::ScheduleMessage\">ScheduleMessage</a>","synthetic":true,"types":["my_async::schedulers::ScheduleMessage"]},{"text":"impl <a class=\"trait\" href=\"https://doc.rust-lang.org/1.64.0/core/marker/trait.Unpin.html\" title=\"trait core::marker::Unpin\">Unpin</a> for <a class=\"struct\" href=\"my_async/schedulers/struct.Spawner.html\" title=\"struct my_async::schedulers::Spawner\">Spawner</a>","synthetic":true,"types":["my_async::schedulers::Spawner"]},{"text":"impl&lt;T&gt; <a class=\"trait\" href=\"https://doc.rust-lang.org/1.64.0/core/marker/trait.Unpin.html\" title=\"trait core::marker::Unpin\">Unpin</a> for <a class=\"struct\" href=\"my_async/schedulers/struct.JoinHandle.html\" title=\"struct my_async::schedulers::JoinHandle\">JoinHandle</a>&lt;T&gt;","synthetic":true,"types":["my_async::schedulers::JoinHandle"]},{"text":"impl&lt;'a, T&gt; <a class=\"trait\" href=\"https://doc.rust-lang.org/1.64.0/core/marker/trait.Unpin.html\" title=\"trait core::marker::Unpin\">Unpin</a> for <a class=\"struct\" href=\"my_async/schedulers/struct.FutureJoin.html\" title=\"struct my_async::schedulers::FutureJoin\">FutureJoin</a>&lt;'a, T&gt;","synthetic":true,"types":["my_async::schedulers::FutureJoin"]},{"text":"impl&lt;T&gt; <a class=\"trait\" href=\"https://doc.rust-lang.org/1.64.0/core/marker/trait.Unpin.html\" title=\"trait core::marker::Unpin\">Unpin</a> for <a class=\"struct\" href=\"my_async/struct.IoWrapper.html\" title=\"struct my_async::IoWrapper\">IoWrapper</a>&lt;T&gt; <span class=\"where fmt-newline\">where<br>&nbsp;&nbsp;&nbsp;&nbsp;T: <a class=\"trait\" href=\"https://doc.rust-lang.org/1.64.0/core/marker/trait.Unpin.html\" title=\"trait core::marker::Unpin\">Unpin</a>,&nbsp;</span>","synthetic":true,"types":["my_async::IoWrapper"]}];
if (window.register_implementors) {window.register_implementors(implementors);} else {window.pending_implementors = implementors;}})()