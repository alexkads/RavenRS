# ravenrs-app

The model of a [RavenRS](https://github.com/alexkads/RavenRS) business application, with no GUI: declare an entity, get an application model with its lists, forms and navigation, and read and write records through an object space.

- `EntityInfo` describes a record: its fields, what they hold and where they show.
- `generate_model` turns entities into the model's lowest layer. `ModelLayers` puts modules, an administrator and a user on top of it, each as a difference in JSON that can be saved, cleared or moved between versions.
- `ObjectSpace` is a unit of work: changes wait for `commit` or `rollback`. `MemoryDatabase` keeps records in memory, on desktop and in the browser.

- `Action` is something people can do, on or off by reasons, by the view it targets, by what is selected and by criteria over it. `ViewController` and `WindowController` bring actions and react to views, and `Frame` holds a view and turns them on and off in a fixed order.
- `Module` and `Application` put it together, and say what is wrong before any screen opens.

Filters, schemas and calculated fields all speak the language of `ravenrs-criteria`.
