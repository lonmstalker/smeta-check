-- Смету теперь можно прислать фотографией: её разбирает нейросеть, а не
-- calamine. Расширение — это же имя файла на диске, поэтому список форматов
-- живёт и здесь: база не должна принимать то, чего код не умеет открыть.
ALTER TABLE estimates DROP CONSTRAINT estimates_file_ext_check;
ALTER TABLE estimates ADD CONSTRAINT estimates_file_ext_check
    CHECK (file_ext IN ('xlsx', 'xls', 'jpg', 'jpeg', 'png', 'webp'));
