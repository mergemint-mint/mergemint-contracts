import React, { useCallback, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { api } from '../lib/api';
import { mapErrorMessage } from '../utils/format';
import { useTranslation } from '../i18n';

export function CreateBounty() {
  const navigate = useNavigate();
  const { t } = useTranslation();
  const [title, setTitle] = useState('');
  const [description, setDescription] = useState('');
  const [reward, setReward] = useState('');
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const handleSubmit = useCallback(
    async (event: React.FormEvent) => {
      event.preventDefault();
      setSubmitting(true);
      setError(null);
      try {
        const bounty = await api.createBounty({ title, description, reward });
        navigate(`/bounties/${bounty.id}`);
      } catch (err) {
        setError(mapErrorMessage(err instanceof Error ? err.message : String(err)));
      } finally {
        setSubmitting(false);
      }
    },
    [title, description, reward, navigate]
  );

  return (
    <form onSubmit={handleSubmit}>
      <input value={title} onChange={(e) => setTitle(e.target.value)} placeholder={t('placeholder_title')} required />
      <textarea
        value={description}
        onChange={(e) => setDescription(e.target.value)}
        placeholder={t('placeholder_description')}
        required
      />
      <input value={reward} onChange={(e) => setReward(e.target.value)} placeholder={t('placeholder_reward')} required />
      {error && <p role="alert">{error}</p>}
      <button type="submit" disabled={submitting}>
        {submitting ? t('creating') : t('create_bounty')}
      </button>
    </form>
  );
}
