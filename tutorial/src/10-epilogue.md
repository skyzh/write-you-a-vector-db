# Epilogue

At this point, you should have learned how to add vector capabilities to an existing relational database system and should have a solid understanding of how vector indexes work. However, there are still some tasks that need to be completed before your vector extension is ready for production. These include persisting the vector indexes to disk, supporting deletion and updates, preventing out-of-memory issues, ensuring efficient index lookups with the disk format, and making the index compatible with the underlying database system, including transactions and multi-version indexes.

## We Need Your Feedback

To help us continously improve the course and enhance your (or other students') learning experience, we sincerely hope that you can tell us something about your learning path. Your feedback is important for us!

[![Join skyzh's Discord Server](discord-badge.svg)](https://skyzh.dev/join/discord)

## Vector Databases or Databases with Vector Extension?

Adding vector capabilities to a database system is relatively easy. In Professor Andy Pavlo's [Databases in 2023: A Year in Review](https://ottertune.com/blog/2023-databases-retrospective) in Ottertune blog, he mentioned that the engineering effort required to introduce a new access method and index data structure for vector search is *not that much*. This tutorial should have demonstrated that extending a database system with vector capabilities can be accomplished without fundamentally altering the system, and by adding additional components, it can easily support vector similarity searches.

> There are two likely explanations for the quick proliferation of vector search indexes. The first is that similarity search via embeddings is such a compelling use case that every DBMS vendor rushed out their version and announced it immediately. The second is that the engineering effort to introduce what amounts to just a new access method and index data structure is small enough that it did not take that much work for the DBMS vendors to add vector search. Most vendors did not write their vector index from scratch and instead just integrated one of the several high-quality open-source libraries available (e.g., Microsoft DiskANN, Meta Faiss). \
> *-- Andy Pavlo, Databases in 2023: A Year in Review on Ottertune Blog*

## Acknowledgments

Personally speaking, vector database is still a new concept to me. I still remember my first involvement with vector databases while interning at Neon in the summer of 2023. One day, Nikita added me to a Slack channel called `#vector`, where I discovered that Konstantin was working on a new vector extension for PostgreSQL called [pgembedding](https://github.com/neondatabase/pg_embedding) incorporating HNSW support. Eventually, this extension got discontinued after pgvector added HNSW support later in 2023. Nevertheless, it was my initial exposure to vector searches and vector databases, and the challenges that developers face when dealing with vector indexes in relational databases are exciting areas for long-term exploration.

At the end of the tutorial, I would like to thank the 15-445 course staff for landing my giant pull request [https://github.com/cmu-db/bustub/pull/682](https://github.com/cmu-db/bustub/pull/682) that adds vector type to the upstream BusTub repo. Thank you Yuchen, Avery, and Ruijie for reviewing my stuff and supporting my work!

{{#include copyright.md}}
