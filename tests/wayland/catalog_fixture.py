"""A local AppStream collection, two desktop categories and a recording launcher."""
import os
from pathlib import Path
from files_fixture import png


def create():
    data = Path(os.environ["XDG_DATA_HOME"])
    # Only these applications participate in discovery. Keep host data out.
    os.environ["XDG_DATA_DIRS"] = str(data)
    apps = data / "applications"
    apps.mkdir()
    handler = Path.home() / "chartroom"
    handler.write_text('#!/bin/sh\ntouch "$HOME/chartroom-launched"\n')
    handler.chmod(0o755)
    (apps / "org.example.Chartroom.desktop").write_text(
        f"[Desktop Entry]\nType=Application\nName=Chartroom\nExec={handler}\nCategories=Science;\n")
    catalog = data / "swcatalog/xml"
    catalog.mkdir(parents=True)
    png(catalog / "soundings.png", 340, 140, lambda x, y:
        (204, 214, 182, 255) if y > 105 - x // 9 + 11 * ((x // 36) % 2)
        else (64 + y // 4, 122 + y // 6, 150 + x // 15, 255))
    (catalog / "survey.xml").write_text("""<components version="1.0">
      <component type="desktop-application"><id>org.example.Chartroom</id>
      <name>Chartroom</name><summary>Plot coastal soundings and compare surveys</summary>
      <description><p>Inspect depth profiles and prepare field notes for a survey crew.</p></description>
      <pkgname>chartroom</pkgname><launchable type="desktop-id">org.example.Chartroom.desktop</launchable>
      <categories><category>Science</category></categories>
      <screenshots><screenshot type="default"><caption>Coastal depth profile</caption><image>soundings.png</image></screenshot></screenshots>
      </component><component type="desktop-application"><id>org.example.Notebook</id><name>Survey notebook</name>
      <summary>Keep research notes together</summary><categories><category>Office</category></categories>
      </component></components>""")
