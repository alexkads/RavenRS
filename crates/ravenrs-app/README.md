# ravenrs-app

The model of a [RavenRS](https://github.com/alexkads/RavenRS) business application, with no GUI: declare an entity, get an application model with its lists, forms and navigation, and read and write records through an object space.

- `EntityInfo` describes a record: its fields, what they hold and where they show.
- `generate_model` turns entities into the model's lowest layer. `ModelLayers` puts modules, an administrator and a user on top of it, each as a difference in JSON that can be saved, cleared or moved between versions.
- `ObjectSpace` is a unit of work: changes wait for `commit` or `rollback`. `MemoryDatabase` keeps records in memory, on desktop and in the browser.

Filters, schemas and calculated fields all speak the language of `ravenrs-criteria`.
