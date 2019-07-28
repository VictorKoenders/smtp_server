SELECT
    mail.id, mail_to.to, mail.from, subject_header.value as subject
FROM mail
LEFT JOIN mail_header AS subject_header ON subject_header.mail_id = mail.id AND subject_header.key ILIKE 'subject'
LEFT JOIN mail_to ON mail_to.mail_id = mail.id;
