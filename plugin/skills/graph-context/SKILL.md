---
name: graph-context
description: Query the ctx context graph for prior session context. Use at the start of a session or when the user asks about past work, related sessions, or prior decisions.
allowed-tools: Bash(ctx *)
---

# Pull context from the context graph

Search the ctx graph for relevant prior work. Use when starting a new task or when the user references past sessions.

## Approach

1. Search for relevant sessions and highlights:
   ```
   ctx search "<relevant terms>"
   ctx find Session tool=claude --limit 10
   ctx find Highlight kind=decision
   ctx find Highlight kind=blocker
   ctx find Highlight kind=todo
   ```

2. If you find a related session, walk its connections:
   ```
   ctx get Session:<id>
   ctx walk Session:<id> HAS_HIGHLIGHT
   ctx walk Session:<id> HAS_TOPIC
   ```

3. Summarize what you found before proceeding with the task.

## Traversal patterns

Find sessions sharing topics with a given session:
```
ctx walk Session:<id> HAS_TOPIC/'~HAS_TOPIC'
```

Find all topics covered across a project:
```
ctx walk Session:<id> IN_PROJECT/'~IN_PROJECT'/HAS_TOPIC
```

Trace a continuation chain:
```
ctx walk Session:<id> CONTINUES
ctx walk Session:<id> '~CONTINUES'
ctx path Session:<from> Session:<to> --via CONTINUES
```

Find what sessions and highlights reference an artifact:
```
ctx walk Artifact:<id> '~MENTIONS'
ctx walk Artifact:<id> '~MENTIONS/~HAS_HIGHLIGHT'
```

See how topics relate to each other:
```
ctx walk Topic:<id> RELATED_TO
ctx get Topic:<id>
```

Find everything connected to a node (any edge, one hop):
```
ctx walk Session:<id> '*'
```

## Filters

```
ctx find Session tool=claude,model=opus
ctx find Session 'title~auth'
ctx find Session 'title^Fix'
ctx find Highlight kind=blocker
ctx find Highlight 'content~performance'
ctx find Artifact kind=pr
```

## Aggregation

```
ctx count Session --by tool
ctx count Session --by model
ctx count Highlight --by kind
ctx kinds
ctx stats
```

## Schema reference

Run `ctx schema <Kind>` to see properties and hints for any node type. Available kinds: Session, Topic, Highlight, Artifact, Project, Branch, Model.

$ARGUMENTS
