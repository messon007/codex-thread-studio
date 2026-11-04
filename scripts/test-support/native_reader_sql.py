"""SQL case for the shared installed native reader-query fixture."""

def exercise(client,root,entry,owned,keys,tr):
    import gi
    from gi.repository import Gio
    title=tr('Query table with SQL')
    scope=None
    frame=lambda:client.find(lambda n:n.get_role_name()=='frame' and n.get_name()==title)
    def inside(node):return scope is not None and client.within(node,scope) and client.showing(node)
    def click(name):return client.click(tr(name),parent=scope)
    def label(value):return client.find(lambda n:n.get_role_name()=='label' and n.get_name()==value and inside(n))
    def field():return client.find(lambda n:n.get_role_name()=='text' and n.get_name()==tr('SQL query') and inside(n))
    def focus():
        for _ in range(6):
            current=scope
            if current:current.clear_cache()
            if current and current.get_state_set().contains(client.api.StateType.ACTIVE):return
            keys([0xffe9,0xff09])
        client.wait('owned SQL frame active',lambda:frame() and frame().get_state_set().contains(client.api.StateType.ACTIVE))
    def query(text):client.edit(client.wait('SQL editor',field),text);click('Query')
    def name(index):return ('SQL row-%04d 中文😀 '%index+'context '*12).rstrip()
    source=root/'project/query table 中文.md'
    assert entry.launch([Gio.File.new_for_path(str(source))],None)
    client.wait('native Markdown table',lambda:client.find(lambda n:n.get_name()==title and n.get_role_name()=='button' and client.showing(n) and client.sensitive(n)))
    client.click(title);scope=client.wait('native SQL window',frame)
    client.wait('complete table projection ready',lambda:label(name(0)))
    editor=client.wait('SQL editor',field);initial=client.contents(editor)
    assert '*' not in initial and '\n' in initial and '"Name"' in initial and '"_line"' in initial
    click('Compact');client.wait('shared SQL compact formatting',lambda:client.contents(editor).count('\n')<initial.count('\n'))
    sql='SELECT "Name", CAST("Value" AS INTEGER) AS number, CASE WHEN "Status"=\'keep\' THEN \'保留😀\' ELSE NULL END AS flag FROM data WHERE "Status"=\'keep\' ORDER BY number DESC'
    query(sql);client.wait('SQL filtering and numeric sorting',lambda:label(name(1198)))
    click('Copy');client.wait('independent SQL page clipboard',lambda:owned.clipboard_text().startswith('Name\tnumber\tflag\n'+name(1198)+'\t1198\t保留😀'))
    click('Next result page');client.wait('second SQL result page',lambda:label(name(1098)))
    click('Previous result page');client.wait('previous SQL result page',lambda:label(name(1198)))
    focus()
    # The existing shared picker names columns by their original alias. Its
    # GTK check nodes can require normal keyboard navigation on GTK 4.22.
    columns=tr('Columns {visible}/{total}').replace('{visible}','3').replace('{total}','3')
    client.click(columns,parent=scope)
    client.toggle('flag',keys,parent=scope);keys([0xff1b])
    client.wait('SQL output column hidden',lambda:not label('flag'))
    client.click(tr('Columns {visible}/{total}').replace('{visible}','2').replace('{total}','3'),parent=scope)
    client.toggle('flag',keys,parent=scope);keys([0xff1b])
    client.wait('SQL output column restored',lambda:label('flag'))
    # A literal long value makes row-height changes observable through the
    # public component interface rather than inspecting application state.
    long=client.wait('wrapped SQL value',lambda:label(name(1198)))
    height=lambda:client.api.Component.get_extents(long.get_component_iface(),client.api.CoordType.WINDOW).height
    wrapped=height();click('Wrap');client.wait('SQL no-wrap row height',lambda:height()<wrapped)
    click('Wrap');client.wait('SQL wrap row height restored',lambda:height()>=wrapped)
    query('DROP TABLE data')
    client.wait('SQL write rejected',lambda:client.find(lambda n:n.get_role_name()=='label' and 'authorized' in n.get_name().lower() and inside(n)))
    assert label(name(1198)),'SQL error discarded the successful read-only result'
    click('Reset query');client.wait('SQL reset restores all rows',lambda:label(name(0)))
    assert '"Name"' in client.contents(editor)
    # The source reader uses its ordinary Save/Edit lifetimes; selecting a
    # different document must retire the SQL dialog and its old table scope.
    # Replace the Markdown reader's source, rather than navigating to the
    # separate structured workspace, which intentionally keeps its parent
    # document available for Back navigation.
    replacement=root/'project/query replacement 中文.md'
    assert entry.launch([Gio.File.new_for_path(str(replacement))],None)
    client.wait('old SQL source retired',lambda:frame() is None)
    client.wait('replacement Markdown source',lambda:client.text(replacement.name))
    return dict(nativeSqlControls=True,sharedSqlFormatting=True,defaultNamedProjection=True,fullTableFilterSortCase=True,pagedResults=True,independentWaylandCopy=True,sharedColumnPicker=True,wrapAdjustsRowHeight=True,errorKeepsResult=True,resetQuery=True,sourceChangeRetiresQuery=True)
