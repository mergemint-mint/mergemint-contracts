import React, { useEffect } from 'react';
import { BrowserRouter, Link, Route, Routes, useLocation } from 'react-router-dom';
import { WalletProvider, useWallet } from './lib/WalletContext';
import { WalletConnectButton } from './components/WalletConnectButton';
import { NetworkMismatchBanner } from './components/NetworkMismatchBanner';
import { BountyList } from './pages/BountyList';
import { BountyDetail } from './pages/BountyDetail';
import { CreateBounty } from './pages/CreateBounty';
import { ContributorProfile } from './pages/ContributorProfile';
import { BountyErrorBoundary } from './components/BountyErrorBoundary';
import { useTranslation } from './i18n';

function Nav() {
  const location = useLocation();
  const { address, connect, clearError } = useWallet();
  const { t } = useTranslation();

  // A prior connect() failure otherwise stays visible until the next
  // connect() attempt, even after navigating away (issue #508).
  useEffect(() => {
    clearError();
  }, [location.pathname, clearError]);

  return (
    <nav>
      <Link to="/">{t('nav_bounties')}</Link>
      <Link to="/create">{t('nav_create_bounty')}</Link>
      <WalletConnectButton address={address} onConnect={connect} />
    </nav>
  );
}

export default function App() {
  return (
    <WalletProvider>
      <BrowserRouter>
        <Nav />
        <NetworkMismatchBanner />
        <Routes>
          <Route
            path="/"
            element={
              <BountyErrorBoundary>
                <BountyList />
              </BountyErrorBoundary>
            }
          />
          <Route
            path="/bounties/:id"
            element={
              <BountyErrorBoundary>
                <BountyDetail />
              </BountyErrorBoundary>
            }
          />
          <Route
            path="/create"
            element={
              <BountyErrorBoundary>
                <CreateBounty />
              </BountyErrorBoundary>
            }
          />
          <Route
            path="/contributors/:address"
            element={
              <BountyErrorBoundary>
                <ContributorProfile />
              </BountyErrorBoundary>
            }
          />
        </Routes>
      </BrowserRouter>
    </WalletProvider>
  );
}
