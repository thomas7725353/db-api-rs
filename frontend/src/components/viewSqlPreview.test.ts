import { describe, expect, it } from 'vitest';
import { renderViewSqlPreview } from './viewSqlPreview';

describe('renderViewSqlPreview', () => {
  it('renders safe identifiers and integer paging fragments', () => {
    const preview = renderViewSqlPreview(
      'select [[ columns | ident_list ]] from demo_items a order by [[ order_by | ident ]] desc limit [[ limit | int(default=10,max=1000) ]] offset [[ offset | int(default=0) ]]',
      {
        columns: ['a.id', 'a.name', 'a.c7'],
        order_by: 'a.c7',
        limit: 20,
      },
    );

    expect(preview.sql).toBe(
      'select a.id, a.name, a.c7 from demo_items a order by a.c7 desc limit 20 offset 0',
    );
  });

  it('rejects unsafe identifiers in preview', () => {
    expect(() =>
      renderViewSqlPreview('select [[ columns | ident_list ]] from demo_items', {
        columns: ['id', 'name; drop table demo_items'],
      }),
    ).toThrow(/Invalid identifier/);
  });
});
